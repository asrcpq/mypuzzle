use tttz_protocol::{Display, KeyType, ServerMsg, ClientMsg};
use tttz_libclient::client_socket::ClientSocket;

use std::collections::VecDeque;

pub trait Thinker {
	fn main_think(display: &Display) -> VecDeque<KeyType>;

	fn main_loop(addr: &str, sleep_millis: u64, strategy: bool) {
		let (client_socket, id) = ClientSocket::new(&addr);
		let main_sleep = 10;
	
		let mut state = 3;
		let mut last_display: Option<Display> = None;
		let mut moveflag = false;
		let mut operation_queue: VecDeque<KeyType> = VecDeque::new();
		loop {
			std::thread::sleep(std::time::Duration::from_millis(main_sleep));
			// read until last screen
			while let Ok(server_msg) = client_socket.recv() {
				match server_msg {
					ServerMsg::Display(display) => {
						if display.id == id {
							last_display = Some(display.into_owned());
						} else {
							// strategy ai moves after user move
							if strategy {
								moveflag = true;
							}
						}
					}
					ServerMsg::GameOver(_) => {
						state = 1;
					}
					ServerMsg::Start(_) => {
						state = 2;
					}
					ServerMsg::Request(id) => {
						state = 2;
						client_socket.send(ClientMsg::Accept(id)).unwrap();
					}
					ServerMsg::Terminate => {
						return;
					}
					_ => eprintln!("Skipping msg: {}", server_msg),
				}
			}
			if strategy {
				if let Some(ref decoded) = last_display {
					if state == 2 && moveflag {
						if operation_queue.is_empty() {
							operation_queue = Self::main_think(decoded);
						}
						client_socket
							.send(ClientMsg::KeyEvent(
								operation_queue.pop_front().unwrap(),
							))
							.unwrap();
						moveflag = false;
						last_display = None;
					}
				}
			} else if let Some(ref decoded) = last_display {
				if state == 2 {
					operation_queue = Self::main_think(decoded);
					while let Some(key_type) = operation_queue.pop_front() {
						client_socket.send(ClientMsg::KeyEvent(key_type)).unwrap();
						std::thread::sleep(std::time::Duration::from_millis(
							sleep_millis,
						));
					}
				}
				last_display = None;
			}
		}
	}
}
