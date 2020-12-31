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

const HASH_WINDOW_SIZE: usize = 2 << 16;
const HASH_AHEAD: usize = MAX_REP_LEN; // we need to calc a little ahead because some repetitions might continue to in front of reps_tracker.pos;

struct RepsTracker<'a> {
	data: &'a [u8],
	pos: usize, // current position in the data
	reps: HashMap<&'a [u8], VecDeque<usize>>, // maps [u8; 3] to their positions, closest in front, too far are discarded.
	to_forget: VecDeque<&'a	[u8]>, // remember who was where so you could discard far ones. newest in front.
	window_rolling_hash: [u32; HASH_WINDOW_SIZE], // remember hash of recently terminated(+a little in the future) data prefixes. cyclic. hash of data[0..x] in x % size.
}

impl RepsTracker<'_> {
	fn new(data: &[u8]) -> RepsTracker {
		let mut s = RepsTracker {
			data,
			pos: 0,
			reps: HashMap::new(),
			to_forget: VecDeque::new(),
			window_rolling_hash: [0; HASH_WINDOW_SIZE],
		};
		for i in 0..HASH_AHEAD {
			s.window_rolling_hash[i + 1] = RepsTracker::extend_hash(s.window_rolling_hash[i], s.data[i]);
		}
		s
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
		if self.pos + HASH_AHEAD < self.data.len() {
			let b = self.data[self.pos + HASH_AHEAD];
			let prev_hash = self.window_rolling_hash[(self.pos + HASH_AHEAD) % HASH_WINDOW_SIZE];
			let next_hash = RepsTracker::extend_hash(prev_hash, b);
			self.window_rolling_hash[(self.pos + HASH_AHEAD + 1) % HASH_WINDOW_SIZE] = next_hash;
		}
		self.pos += 1;
		if self.to_forget.len() > MAX_REP_DIST {
			// discard old ones
			let too_old = self.to_forget.pop_back().unwrap();
			self.reps.get_mut(too_old).unwrap().pop_back();
		} 
	}

	fn hash_recent_substring(&self, a: usize, b: usize) -> u32 {
		// return hash of self.data[a..b]
		// assumes the hashes of [..a], [..b] are still in the window/hash ahead.
		self.window_rolling_hash[a % HASH_WINDOW_SIZE].rotate_left((b - a) as u32) ^ self.window_rolling_hash[b % HASH_WINDOW_SIZE]
	}

	fn extend_hash(hash: u32, b: u8) -> u32 {
		hash.rotate_left(1) ^ BUZHASH_TABLE[b as usize]
	}

	fn get_reps(&self) -> Vec<(usize, u32)> {
		// finds old occurrences of upcoming bytes.
		// returns a Vec of tuples of the form: (rep dist, length)
		// closer reps are first, only returns the closest one of each length
		let max_len = |start: usize, min_len_to_care: usize| -> u32 {
			// how many bytes of the sequence starting at start agree with the one at pos?
			// if we are sure that the result < min_len_to_care then we can return 0

			if unsafe { self.data.get_unchecked(start + min_len_to_care - 1) != self.data.get_unchecked(self.pos + min_len_to_care - 1) } {
				// we definitely don't care. if this reads outside of data, then anyway a rep we care about can't exist.
				return 0
			}
			// using hash to check if it is possible that we care
			let hash1 = self.hash_recent_substring(start, start + min_len_to_care);
			let hash2 = self.hash_recent_substring(self.pos, self.pos + min_len_to_care);
			if hash1 != hash2 {
				// we definitely don't care.
				return 0
			}

			let mut res: usize = 3; // only called if 3 is already known
			// bound check manually perfomed later. this is first because it is a lot more likely to return false
			while unsafe { self.data.get_unchecked(start + res) == self.data.get_unchecked(self.pos + res) } &&
			res < MAX_REP_LEN &&
			self.pos + res < self.data.len() { // no need to check start + res since start < self.pos
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
			let l = max_len(*start, longest as usize + 1);
			if l > longest {
				out.push((self.pos - *start, l));
				longest = l;
			}
			if longest == MAX_REP_LEN as u32 {
				break;
			}
		}
		out
	}
}


// random values.
const BUZHASH_TABLE: [u32; 256] = [
0x012e00ee, 0x186a4024, 0x1907cb61, 0x11ec7b23, 0x3ad46cc4, 0x71162b15, 0x0e249d5b, 0x4b3da547,
0x47fcc0ba, 0x23563a52, 0x7ff790f7, 0x2e4fc914, 0x0be8bcb7, 0x46d8f54c, 0x74792284, 0x1b80331e,
0x41b1ef25, 0x370de29e, 0x7182e115, 0x096b1c25, 0x206405cf, 0x27f90686, 0x1f8cfa96, 0x44a81987,
0x0b68380a, 0x0ac9b87d, 0x141a7085, 0x1405d28d, 0x65f1bb41, 0x2df529c6, 0x79e53f7f, 0x7ae0b359,
0x2067b2ac, 0x02f02fb1, 0x6e04c595, 0x7bc101a4, 0x4079bf1e, 0x00cf7d8e, 0x52b73c08, 0x5ea7809b,
0x2c886d78, 0x2a100e94, 0x1b884f7d, 0x34fc924d, 0x35b645a6, 0x3330a761, 0x707d15cb, 0x6e1066e8,
0x6b4dd29f, 0x5860b7ae, 0x054ee9d8, 0x4550a270, 0x06d8a15c, 0x17c8c917, 0x5cfdcccc, 0x1a560f8e,
0x6f039ed3, 0x446356f3, 0x6ace7f3b, 0x14054ec6, 0x7a0c0a9c, 0x35b770e7, 0x19f98813, 0x4ccea734,
0x0689da7b, 0x43ddc707, 0x39ea8fdc, 0x1d311ad9, 0x245b3011, 0x731c377c, 0x3fb245b0, 0x3bfe0047,
0x6f7b824f, 0x7a9a56ba, 0x06c7337b, 0x752402c6, 0x6a0e8cd6, 0x2a3646b8, 0x2232dbcb, 0x58bacce9,
0x312b27c0, 0x348eac94, 0x7f2a0793, 0x16c1d72f, 0x57da0783, 0x1bf77024, 0x4e2bf5ce, 0x6c079746,
0x1fed0a3a, 0x1edb4ca8, 0x6fa4499f, 0x070157f9, 0x5bb60213, 0x7e754c72, 0x277e084a, 0x646784d4,
0x4bd17891, 0x314dd4b9, 0x6754d65f, 0x31f34921, 0x5b1cc1cd, 0x7682cc86, 0x66a1c7e4, 0x79bf95bd,
0x6cce84d3, 0x6bbdaa72, 0x4321e33c, 0x10b8bf51, 0x37c7ef99, 0x59649277, 0x2a697242, 0x4ba110f1,
0x7b6405b8, 0x2438288e, 0x62337a9b, 0x3f3cd136, 0x68a644fd, 0x51c7ddcd, 0x6cdf1b2e, 0x35a2ae29,
0x0b9ffe23, 0x2d0304c5, 0x47238bc6, 0x3efa78fa, 0x5318b2bd, 0x7201abba, 0x1a4113d8, 0x2ced6ef3,
0x6d8e7768, 0x73abfab8, 0x0d7140d8, 0x2ed4f953, 0x79f44b62, 0x0219e80c, 0x0722ad1d, 0x3bb09944,
0x7d44d7aa, 0x79db81e4, 0x3ae788c6, 0x4ae430f1, 0x45871ecf, 0x53c814ff, 0x352da8b5, 0x0287153e,
0x5a43275d, 0x40c12a36, 0x7124fddc, 0x5c854523, 0x67ee4e6c, 0x03aabf25, 0x2d6ab386, 0x526c4a86,
0x43c2a5f8, 0x6695634f, 0x3bcc098f, 0x2c21a9c1, 0x7199aa2a, 0x0901ca9f, 0x08cb9170, 0x14fd90f1,
0x5876cfcd, 0x6c4748e2, 0x562ea892, 0x5ae69f42, 0x100658c2, 0x1d421d99, 0x48db5d4b, 0x08a79aac,
0x07ddcb89, 0x18546c37, 0x5649e78b, 0x6a67147f, 0x2fd3e018, 0x70edb23e, 0x672bd731, 0x1b4d2b03,
0x7b36d2d2, 0x396cc567, 0x5885244a, 0x2aa52cf8, 0x47bb6821, 0x4d2d9be6, 0x2e0e2784, 0x09303310,
0x59d3ef77, 0x152ec414, 0x14234dc8, 0x5386366e, 0x0d65d415, 0x13b2b09e, 0x1fe7305c, 0x148a6c38,
0x11c44c99, 0x7983507a, 0x1d2c8710, 0x6e7ba070, 0x0e5ab9b9, 0x43005097, 0x6a2b64b2, 0x0f1de3bb,
0x6fe4f43e, 0x494cafbe, 0x4c5cc769, 0x201f8b6f, 0x177f0c54, 0x67692ada, 0x10030291, 0x044e6700,
0x23756fbc, 0x4ac9820b, 0x1a033c94, 0x043a1b13, 0x1e2d39d9, 0x61cef499, 0x591a43ee, 0x70c944e2,
0x272c5241, 0x050798f6, 0x677fee46, 0x03f3c0aa, 0x73d4256d, 0x56ad1a44, 0x45f49e91, 0x6f55955f,
0x3ca7427b, 0x762f8e50, 0x3379c13e, 0x07dd347d, 0x7686cafb, 0x6fd6302e, 0x50b29971, 0x61b2775e,
0x705faab0, 0x36a7a017, 0x4ce101d3, 0x6b3c077d, 0x5ebdc84c, 0x61a794a2, 0x4b5ff558, 0x3c7200c5,
0x0cfece36, 0x51890909, 0x2e4dc125, 0x5dadd8f9, 0x68a677b8, 0x66e3bff1, 0x787840a5, 0x61f42c94,
0x7cb2bbe7, 0x57233ff2, 0x5599bd36, 0x4821a50b, 0x7be642fd, 0x4345a74f, 0x67ca7053, 0x351500ba
];
