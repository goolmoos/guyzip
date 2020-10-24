use std::collections::{HashMap, VecDeque};

use crate::deflate::Token;

const MAX_REP_LEN: usize = 258; // max len supported by the deflate format
const MAX_REP_DIST: usize = 32768; // max dist supported by the deflate format

pub fn lempel_ziv(data: &[u8]) -> Vec<Token> {
	// data.iter().map(|x| Token::Literal(*x as u32)).collect()
	let mut reps_tracker = RepsTracker::new(data);
	let mut out = vec![];
	while reps_tracker.pos < data.len() {
		let reps = reps_tracker.get_reps();
		let best_rep = reps.last(); // longest one
		match best_rep {
			Some((start, length)) => {
				out.push(Token::Repeat(*length, (reps_tracker.pos - start) as u32));
				for _ in 0..*length {
					reps_tracker.advance();
				}
			}
			None => {
				out.push(Token::Literal(data[reps_tracker.pos]));
				reps_tracker.advance();
			}
		};
	}
	out
}

struct RepsTracker<'a> {
	data: &'a [u8],
	pos: usize, // current position in the data
	reps: HashMap<&'a [u8], VecDeque<usize>>, // maps [u8; 3] to their positions, closest in front, too far are discarded.
	to_forget: VecDeque<&'a	[u8]>, // remember who was were so you could discard far ones. newest in front.
}

impl RepsTracker<'_> {
	fn new(data: &[u8]) -> RepsTracker {
		RepsTracker {
			data,
			pos: 0,
			reps: HashMap::new(),
			to_forget: VecDeque::new(),
		}
	}

	fn advance(&mut self) {
		if self.pos + 3 <= self.data.len() {
			// add new ones
			let curr = &self.data[self.pos..self.pos + 3];
			let vec = match self.reps.get_mut(curr) {
				Some(v) => v,
				None => {self.reps.insert(curr, VecDeque::new()); self.reps.get_mut(curr).unwrap()},
			};
			vec.push_front(self.pos);
			self.to_forget.push_front(curr);
		}
		self.pos += 1;
		if self.to_forget.len() > MAX_REP_DIST {
			// discard old ones
			let too_old = self.to_forget.pop_back().unwrap();
			self.reps.get_mut(too_old).unwrap().pop_back();
		} 
	}

	fn get_reps(&self) -> Vec<(usize, u32)> {
		// finds old occurrences of upcoming bytes.
		// returns a Vec of tuples of the form: (rep start, length)
		// closer reps are first, only returns the closest one of each length
		let max_len = |start: usize| -> u32 {
			// how many bytes of the sequence starting at start agree with the one at pos?
			let mut res: usize = 0;
			while res <= MAX_REP_LEN &&
			self.data[start + res] == self.data[self.pos + res] {
				res += 1;
			}
			res as u32
		};
		if self.pos + 3 > self.data.len() {
			return vec![]; // no room left for reps
		}
		let reps = match self.reps.get(&self.data[self.pos..self.pos + 3]) {
			Some(rep_deque) => rep_deque,
			None => return vec![], // no reps
		};
		let mut out = vec![];
		let mut longest = 0;
		for start in reps {
			let l = max_len(*start);
			if l > longest {
				out.push((*start, l));
			}
			longest = l;
		}
		out
	}
}
