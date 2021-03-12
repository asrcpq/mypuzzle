use tttz_protocol::Display;
use tttz_protocol::{BoardMsg, BoardReply, KeyType, SoundEffect};
use tttz_ruleset::*;

use crate::block::Block;
use crate::random_generator::RandomGenerator;
use crate::garbage_attack_manager::GarbageAttackManager;
use crate::replay::Replay;
use rand::Rng;

use std::collections::HashSet;

pub struct Board {
	id: i32,
	pub tmp_block: Block,
	pub shadow_block: Block,
	pub rg: RandomGenerator,
	pub(in crate) color: [[u8; 10]; 40],
	hold: u8,
	pub gaman: GarbageAttackManager,
	pub last_se: SoundEffect,
	pub height: i32,
	pub replay: Replay,
}

impl Board {
	pub fn new(id: i32) -> Board {
		let replay = Default::default();
		let mut board = Board {
			id,
			tmp_block: Block::new(0),    // immediately overwritten
			shadow_block: Block::new(0), // immediately overwritten
			rg: Default::default(),
			color: [[7; 10]; 40],
			hold: 7,
			gaman: Default::default(),
			last_se: SoundEffect::Silence,
			height: 0,
			replay,
		};
		board.spawn_block();
		board.calc_shadow();
		board
	}

	fn is_pos_inside(&self, pos: (i32, i32)) -> bool {
		if pos.0 < 0 || pos.1 < 0 {
			return false;
		}
		if pos.0 >= 10 || pos.1 >= 40 {
			return false;
		}
		true
	}

	pub fn is_pos_vacant(&self, pos: (i32, i32)) -> bool {
		if !self.is_pos_inside(pos) {
			return false;
		}
		self.color[pos.1 as usize][pos.0 as usize] == 7
	}

	// true = die
	pub fn handle_msg(&mut self, board_msg: BoardMsg) -> BoardReply {
		self.replay.push_operation(board_msg.clone());
		let mut atk = 0;
		match board_msg {
			BoardMsg::KeyEvent(key_type) => match key_type {
				KeyType::Nothing => {}
				KeyType::Hold => {
					self.hold();
					self.last_se = SoundEffect::Hold;
				}
				KeyType::Left => {
					self.move1(1);
				}
				KeyType::LLeft => {
					self.move2(1);
				}
				KeyType::Right => {
					self.move1(-1);
				}
				KeyType::RRight => {
					self.move2(-1);
				}
				KeyType::HardDrop => {
					let ret = self.press_up();
					atk = match ret {
						None => return BoardReply::Die,
						Some(atk) => atk,
					}
				}
				KeyType::SoftDrop => {
					let ret = self.press_down();
					atk = match ret {
						None => return BoardReply::Die,
						Some(atk) => atk,
					}
				}
				KeyType::RotateReverse => {
					self.rotate(-1);
				}
				KeyType::Rotate => {
					self.rotate(1);
				}
				KeyType::RotateFlip => {
					self.rotate(2);
				}
			},
			BoardMsg::Attacked(amount) => {
				self.gaman.push_garbage(amount);
				const MAX_GARBAGE_LEN: usize = 5;
				if self.gaman.garbages.len() > MAX_GARBAGE_LEN {
					if self.flush_garbage(MAX_GARBAGE_LEN) {
						return BoardReply::Die;
					} else {
						return BoardReply::GarbageOverflow;
					}
				}
			}
		}
		BoardReply::Ok(atk)
	}

	fn move1(&mut self, dx: i32) -> bool {
		self.tmp_block.pos.0 -= dx;
		if !self.tmp_block.test(self) {
			self.tmp_block.pos.0 += dx;
			return false;
		}
		true
	}

	fn move2(&mut self, dx: i32) {
		while self.move1(dx) {}
	}

	fn rotate2(&mut self, dr: i8) -> u8 {
		let code = self.tmp_block.code;
		let rotation = self.tmp_block.rotation;
		if code == 3 {
			return 0;
		}
		self.tmp_block.rotate(dr);
		let std_pos = self.tmp_block.pos;
		for wkp in kick_iter(code, rotation, dr) {
			self.tmp_block.pos.0 = std_pos.0 + wkp.0 as i32;
			self.tmp_block.pos.1 = std_pos.1 + wkp.1 as i32;
			if self.tmp_block.test(self) {
				if self.test_twist() > 0 {
					return 2;
				} else {
					return 1;
				}
			}
		}
		0
	}

	// rotate2 is extracted for AI
	fn rotate(&mut self, dr: i8) {
		let revert_block = self.tmp_block.clone();
		let ret = self.rotate2(dr);
		if ret == 0 {
			self.tmp_block = revert_block;
		}
		self.last_se = SoundEffect::Rotate(ret);
	}

	fn spawn_block(&mut self) {
		let code = self.rg.get();
		self.replay.push_block(code);
		self.tmp_block = Block::new(code);
	}

	fn hold(&mut self) {
		if self.hold == 7 {
			self.hold = self.tmp_block.code;
			self.spawn_block();
		} else {
			let tmp = self.hold;
			self.hold = self.tmp_block.code;
			self.tmp_block = Block::new(tmp);
		}
	}

	fn soft_drop(&mut self) -> bool {
		if self.shadow_block.pos.1 == self.tmp_block.pos.1 {
			return false;
		}
		self.tmp_block.pos.1 = self.shadow_block.pos.1;
		true
	}

	// return count of lines eliminated
	fn checkline(&self, ln: HashSet<usize>) -> Vec<usize> {
		let mut elims = Vec::new();
		for each_ln in ln.iter() {
			let mut flag = true;
			for x in 0..10 {
				if self.color[*each_ln][x] == 7 {
					flag = false;
				}
			}
			if flag {
				elims.push(*each_ln);
			}
		}
		elims
	}

	fn proc_elim(&mut self, elims: Vec<usize>) {
		let mut movedown = 0;
		for i in 0..40 {
			let mut flag = false;
			for elim in elims.iter() {
				if i == *elim {
					flag = true;
					break;
				}
			}
			if flag {
				movedown += 1;
				continue;
			}
			if movedown == 0 {
				continue;
			}
			self.color[i - movedown] = self.color[i];
		}
	}

	// moving test
	fn test_twist2(&mut self) -> bool {
		self.tmp_block.pos.0 -= 1;
		if self.tmp_block.test(self) {
			self.tmp_block.pos.0 += 1;
			return false;
		}
		self.tmp_block.pos.0 += 2;
		if self.tmp_block.test(self) {
			self.tmp_block.pos.0 -= 1;
			return false;
		}
		self.tmp_block.pos.0 -= 1;
		self.tmp_block.pos.1 += 1;
		if self.tmp_block.test(self) {
			self.tmp_block.pos.1 -= 1;
			return false;
		}
		self.tmp_block.pos.1 -= 1;
		true
	}

	// test all types of twists
	// return 0: none, 1: mini, 2: regular
	fn test_twist(&mut self) -> u32 {
		// No o spin
		if self.tmp_block.code == 3 {
			return 0;
		}
		if !self.test_twist2() {
			return 0;
		}
		// No mini i spin
		if self.tmp_block.code == 0 {
			return 1;
		}
		let tmp = TWIST_MINI_CHECK[self.tmp_block.code as usize]
			[self.tmp_block.rotation as usize];
		for mini_pos in tmp.iter() {
			let check_x = self.tmp_block.pos.0 + mini_pos.0;
			let check_y = self.tmp_block.pos.1 + mini_pos.1;
			if self.color[check_y as usize][check_x as usize] == 7 {
				return 1;
			}
		}
		2
	}

	// true = death
	fn flush_garbage(&mut self, max: usize) -> bool {
		let mut flag = false;
		self.height += self.generate_garbage(max);
		if self.calc_shadow() {
			flag = true;
		}
		if self.height == 40 {
			flag = true;
		}
		flag
	}


	// No plain drop
	fn attack_se(&self, atk: u32, line_clear: u32) -> SoundEffect {
		if line_clear == 0 {
			return SoundEffect::PlainDrop
		}
		let mut se = if atk >= 4 {
			 SoundEffect::AttackDrop2
		} else if atk >= 1 {
			 SoundEffect::AttackDrop
		} else {
			 SoundEffect::ClearDrop
		};
		if self.height == 0 {
			 se = SoundEffect::PerfectClear;
		}
		se
	}

	// set color, update height
	// return lines to check
	fn hard_drop_set_color(&mut self) -> HashSet<usize> {
		let tmppos = self.tmp_block.getpos();
		let mut lines_tocheck = HashSet::new();
		for each_square in tmppos.iter() {
			let px = each_square.0 as usize;
			let py = each_square.1 as usize;
			// tmp is higher, update height
			if py + 1 > self.height as usize {
				self.height = py as i32 + 1;
			}

			// generate lines that changed
			lines_tocheck.insert(py);
			self.color[py][px] = self.tmp_block.code;
		}
		lines_tocheck
	}

	// pull pending garbages and write to board color
	pub fn generate_garbage(
		&mut self,
		keep: usize,
	) -> i32 {
		const SAME_LINE: f32 = 0.6;
		let mut ret = 0;
		loop {
			if self.gaman.garbages.len() <= keep {
				break;
			}
			let mut count = match self.gaman.garbages.pop_front() {
				Some(x) => x,
				None => break,
			} as usize;
			let mut slot = self.rg.rng.gen_range(0..10);
			// assert!(count != 0);
			if count > 40 {
				count = 40;
			}
			ret += count;
			for y in (count..40).rev() {
				for x in 0..10 {
					self.color[y][x] = self.color[y - count][x];
				}
			}
			for y in 0..count {
				let same = self.rg.rng.gen::<f32>();
				if same >= SAME_LINE {
					slot = self.rg.rng.gen_range(0..10);
				}
				for x in 0..10 {
					self.color[y][x] = 2; // L = white
				}
				self.color[y][slot] = 7;
				if !self.tmp_block.test(self) {
					self.tmp_block.pos.1 -= 1;
				}
			}
		}
		ret as i32
	}

	fn hard_drop(&mut self) -> Option<u32> {
		// check twist before setting color
		let twist = self.test_twist();
		let lines_tocheck = self.hard_drop_set_color();

		let elim = self.checkline(lines_tocheck);
		let line_count = elim.len() as u32;
		self.proc_elim(elim);
		// put attack amount into pool
		let atk = self.gaman.calc_attack(
			twist,
			line_count,
			self.tmp_block.code,
			self.height == 0,
		);
		self.last_se = self.attack_se(atk, line_count);
		if line_count > 0 {
			self.height -= line_count as i32;
		} else {
			// plain drop: attack execution
			self.height += self.generate_garbage(0);
			if self.height == 40 {
				return None;
			}
		}

		// new block
		self.spawn_block();
		if self.calc_shadow() {
			return None;
		}
		Some(atk)
	}

	// true = death
	fn press_down(&mut self) -> Option<u32> {
		if !self.soft_drop() {
			return self.hard_drop();
		} else {
			self.last_se = SoundEffect::SoftDrop;
		}
		Some(0)
	}

	// true = death
	fn press_up(&mut self) -> Option<u32> {
		self.soft_drop();
		self.hard_drop()
	}

	// true: die
	pub fn calc_shadow(&mut self) -> bool {
		self.shadow_block = self.tmp_block.clone();
		loop {
			self.shadow_block.pos.1 -= 1;
			if !self.shadow_block.test(self) {
				self.shadow_block.pos.1 += 1;
				break self.shadow_block.pos.1 >= 20;
			}
		}
	}

	pub fn generate_display(&self) -> Display {
		let mut display = Display::new(self.id);
		for i in 0..20 {
			display.color[i] = self.color[i];
		}
		display.shadow_block = self.shadow_block.compress();
		display.tmp_block = self.tmp_block.compress();
		display.garbages = self.gaman.garbages.clone();
		display.hold = self.hold;
		display.tmp_block = self.tmp_block.compress();
		display.garbages = self.gaman.garbages.clone();
		display.cm = self.gaman.cm;
		display.tcm = self.gaman.tcm;
		for i in 0..6 {
			display.bag_preview[i] = self.rg.bag[i];
		}
		display
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::test::*;

	#[test]
	fn test_is_pos_inside() {
		let board = Board::new(1);
		assert_eq!(board.is_pos_inside((10, 40)), false);
		assert_eq!(board.is_pos_inside((10, 5)), false);
		assert_eq!(board.is_pos_inside((0, 0)), true);
		assert_eq!(board.is_pos_inside((4, 20)), true);
	}

	#[test]
	fn test_test_twist() {
		let mut board =
			test::generate_solidlines([2, 3, 0, 2, 0, 0, 0, 0, 0, 0]);
		board.color[39][1] = 7; // sdp: (1, 0)
		board.tmp_block = Block::new(1); // █▄▄
		board.tmp_block.pos.0 = 1;
		board.tmp_block.pos.1 = 0;
		board.tmp_block.rotation = 3;
		assert_eq!(board.test_twist(), 2);
	}

	#[test]
	fn test_calc_shadow() {
		let mut board =
			test::generate_solidlines([1, 3, 2, 5, 4, 1, 2, 5, 2, 0]);
		board.tmp_block = Block::new(1); // █▄▄
		assert!(!board.calc_shadow());
		use std::collections::HashSet;
		let mut blocks: HashSet<(i32, i32)> = HashSet::new();
		blocks.insert((3, 6));
		blocks.insert((3, 5));
		blocks.insert((4, 5));
		blocks.insert((5, 5));
		let shadow_pos = board.shadow_block.getpos();
		println!("{:?} {:?}", blocks, shadow_pos);
		for i in 0..4 {
			blocks.remove(&(shadow_pos[i].0 as i32, shadow_pos[i].1 as i32));
		}
		assert!(blocks.is_empty());
		board.move2(-1); // move to very left
		assert!(!board.calc_shadow());

		blocks.insert((0, 4));
		blocks.insert((0, 3));
		blocks.insert((1, 3));
		blocks.insert((2, 3));
		let shadow_pos = board.shadow_block.getpos();
		println!("{:?} {:?}", blocks, shadow_pos);
		for i in 0..4 {
			blocks.remove(&(shadow_pos[i].0 as i32, shadow_pos[i].1 as i32));
		}
	}
}
