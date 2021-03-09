extern crate tttz_mpboard;
extern crate tttz_protocol;
use crate::client_manager::ClientManager;
use crate::server::SOCKET;
use std::borrow::Cow;
use std::collections::HashSet;
use std::net::SocketAddr;
use tttz_mpboard::board::Board;
use tttz_protocol::{BoardMsg, BoardReply, KeyType, ServerMsg, SoundEffect};

pub struct Client {
	pub id: i32,
	addr: SocketAddr,
	pub dc_ids: HashSet<i32>,
	// 1: waiting
	// 2: in-game
	// 3: pairing
	pub state: i32,
	pub board: Board,
	pub attack_target: i32,
	pub display_update: bool,
}

impl Client {
	pub fn new(id: i32, addr: SocketAddr) -> Client {
		Client {
			id,
			addr,
			dc_ids: HashSet::new(),
			state: 1,
			board: Board::new(id),
			attack_target: 0,
			display_update: true,
		}
	}

	pub fn init_board(&mut self) {
		self.board = Board::new(self.id);
	}

	pub fn pair_success(&mut self, target_id: i32) {
		self.state = 2;
		self.dc_ids.insert(target_id);
		self.attack_target = target_id;
		self.init_board();
		self.send_msg(ServerMsg::Start(target_id));
	}

	pub fn send_msg(&self, msg: ServerMsg) {
		SOCKET.send_to(&msg.serialized(), self.addr).unwrap();
	}

	pub fn broadcast_msg(
		&self,
		client_manager: &ClientManager,
		msg: &ServerMsg,
	) {
		// TODO maintain the view list in client, when exit do cleaning
		for dc_id in self.dc_ids.iter() {
			let dc_addr =
				if let Some(addr) = client_manager.get_addr_by_id(*dc_id) {
					addr
				} else {
					eprintln!(
						"A removed client: {} was viewing {}",
						dc_id, self.id
					);
					continue;
				};
			SOCKET.send_to(&msg.serialized(), dc_addr).unwrap();
		}
	}

	pub fn send_display(&mut self, client_manager: &ClientManager) {
		self.broadcast_msg(
			client_manager,
			&ServerMsg::Display(Cow::Borrowed(&self.board.display)),
		);
		let last_se = std::mem::replace(&mut self.board.last_se, SoundEffect::Silence);
		self.broadcast_msg(
			client_manager,
			&ServerMsg::SoundEffect(self.id, last_se),
		);
	}

	// true = die
	pub fn process_key(&mut self, key_type: KeyType) -> bool {
		if self.state != 2 {
			// not playing
			return false;
		}
		if BoardReply::Die
			== self.board.handle_msg(BoardMsg::KeyEvent(key_type))
		{
			return true;
		}
		// return value ignored, only board change cause death
		self.board.calc_shadow();
		false
	}
}

impl Drop for Client {
	fn drop(&mut self) {
		self.send_msg(ServerMsg::Terminate);
	}
}
