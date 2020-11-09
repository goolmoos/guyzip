use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::deflate::{Token, deflate_code_of_len, deflate_code_of_dist};

const MAX_REP_LEN: usize = 258; // max len supported by the deflate format
const MAX_REP_DIST: usize = 32768; // max dist supported by the deflate format

pub fn lempel_ziv(data: &[u8]) -> Vec<Token> {
	return Encoder::new(data).run();
}

struct Encoder<'a> {
	/*
	Encodes the data in order.
	The possible encodings are saved as a linked list of tokens,
	that splits at any decision(which repetition/literal to use).
	Example with 3 possible encodings:

           token2 - token3 - token4
	      /
	token1                                   option1 for next token
	      \                                 /
	       token2alternative - another token
	                                        \
	                                         options2 for next token

	At any moment, only the best possibilty for each length is stored.
	If we find another, we keep the better one.
	The list heads are saved in a ring buffer with length MAX_REP_LEN.
	For every i from 0 to MAX_REP_LEN,
	at possible_encodings[(i + pos) % MAX_REP_LEN], a tuple may be stored,
	containing the best encoding for the first (i + pos) bytes (yet) and its estimated size (in bits).
	*/
	data: &'a[u8],
	reps_tracker: RepsTracker<'a>,
	possible_encodings: Vec<Option<(TokenList, u64)>>, // length of MAX_REP_LEN.
}

impl Encoder<'_> {
	fn run(mut self) -> Vec<Token> {
		// Return an encoding of the data using deflate::Token. (Literal bytes and repetitions).
		if self.data.len() == 0 {
			return vec![];
		}
		self.possible_encodings[1] = Some((TokenList { token: Token::Literal(self.data[0]), prev: None }, 0));
		self.reps_tracker.advance();

		while self.reps_tracker.pos < self.data.len() {
			if self.reps_tracker.pos % 1024 == 0 {
				let taken: u32 = self.possible_encodings.iter().map(|x| match x {None => 0, Some(_) => 1,}).sum();
				println!("progress: {}, taken: {}", self.reps_tracker.pos, taken);
			}
			let i = self.reps_tracker.pos % MAX_REP_LEN;
			let (curr_encoding, curr_size) = self.possible_encodings[i].take().unwrap();
			let curr_encoding = Rc::new(curr_encoding);

			// could use a literal type token for next byte
			self.insert_next(&curr_encoding, curr_size, Token::Literal(self.data[self.reps_tracker.pos]));

			// could use a repeat token for next bytes
			for (dist, len) in self.reps_tracker.get_reps() {
				self.insert_next(&curr_encoding, curr_size, Token::Repeat(len, dist as u32));
			}
			self.reps_tracker.advance();
		}

		// only one list head should remain.
		let list_head = self.possible_encodings[self.reps_tracker.pos % MAX_REP_LEN].take().unwrap();
		let mut curr = list_head.0;
		let mut out = vec![];

		loop {
			out.push(curr.token);
			match curr.prev {
				None => break,
				Some(prev_ref) => curr = Rc::try_unwrap(prev_ref).unwrap_or_else(|_| panic!()),
			}
		}
		out.reverse();

		out
	}

	fn new(data: &[u8]) -> Encoder {
		let mut possible_encodings = Vec::with_capacity(MAX_REP_LEN);
		for _ in 0..MAX_REP_LEN {
			possible_encodings.push(None);
		}
		Encoder {
			data,
			reps_tracker: RepsTracker::new(data),
			possible_encodings,
		}
	}

	fn insert_next(&mut self, curr_encoding: &Rc<TokenList>, curr_size: u64, next_token: Token) {
		let extra_length = match next_token { Token::Literal(_) => 1, Token::Repeat(len, _dist) => len, };
		let extra_size = size_of_token(&next_token);
		let i = (self.reps_tracker.pos + extra_length as usize) % MAX_REP_LEN;
		let should_insert = match &self.possible_encodings[i] {
			None => true,
			Some((_other, other_size)) => curr_size + extra_size < *other_size,
		};
		if should_insert {
			let next_encoding = TokenList { token: next_token, prev: Some(Rc::clone(curr_encoding)) };
			let next_size = curr_size + extra_size;
			self.possible_encodings[i] = Some((next_encoding, next_size));
		}
	}
}

struct TokenList {
	token: Token,
	prev: Option<Rc<TokenList>>,
}

fn size_of_token(token: &Token) -> u64 {
	// heuristic for size. (depends on what the huffman codes will be).
	match token {
		Token::Literal(_) => 8,
		Token::Repeat(len, dist) => ((deflate_code_of_len(*len).1 + 8) + (deflate_code_of_dist(*dist).1 + 5)) as u64,
	}
}

struct RepsTracker<'a> {
	data: &'a [u8],
	pos: usize, // current position in the data
	reps: HashMap<&'a [u8], VecDeque<usize>>, // maps [u8; 3] to their positions, closest in front, too far are discarded.
	to_forget: VecDeque<&'a	[u8]>, // remember who was where so you could discard far ones. newest in front.
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
		// returns a Vec of tuples of the form: (rep dist, length)
		// closer reps are first, only returns the closest one of each length
		let max_len = |start: usize| -> u32 {
			// how many bytes of the sequence starting at start agree with the one at pos?
			let mut res: usize = 0;
			while res < MAX_REP_LEN &&
			start + res < self.data.len() &&
			self.pos + res < self.data.len() &&
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
				out.push((self.pos - *start, l));
			}
			longest = l;
		}
		out
	}
}
