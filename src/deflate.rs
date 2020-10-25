use std::io::Write;

use crate::huffman;
mod lempel_ziv;

pub enum Token {
	Literal(u8),
	Repeat(u32, u32),
}

pub fn deflate<T: Write>(file: &[u8], out: &mut T) {
	let tokens = lempel_ziv::lempel_ziv(file);
	let mut writer = DeflateWriter::new(out);

	writer.new_fixed_codes_block(true);
	tokens.iter().for_each(|t| writer.write(t));
}

struct DeflateWriter<'a, T: Write> {
	out: &'a mut T,
	curr_bytes: u32,
	curr_full_bits: u8,
	tree: huffman::Tree,
	in_block: bool,
}

impl<'a, T: Write> DeflateWriter<'a, T> {
	fn new(out: &'a mut T) -> DeflateWriter<'a, T> {
		DeflateWriter {
			out,
			curr_bytes: 0,
			curr_full_bits: 0,
			tree: vec![],
			in_block: false,
		}
	}

	fn write_bits(&mut self, bits: u32, len: u8) {
		// packs from LSB to MSB
		// 16 bit max
		self.curr_bytes |= bits << self.curr_full_bits;
		self.curr_full_bits += len;
		while self.curr_full_bits >= 8 {
			self.out.write_all(&[(self.curr_bytes & 0xFF) as u8]).unwrap();
			self.curr_bytes >>= 8;
			self.curr_full_bits -= 8;
		}
	}

	fn write_huffman(&mut self, x: u32){
		let code = self.tree[x as usize];
		self.write_bits(code.code, code.length);
	}

	fn write(&mut self, token: &Token) {
		match token {
			Token::Literal(value) => self.write_huffman(*value as u32),
			Token::Repeat(len, dist) => {
				for (len_start, len_end, extra_bits, code) in &LEN_TO_CODE {
					if len < len_end {
						self.write_huffman(*code);
						self.write_bits(len - len_start, *extra_bits);
						break;
					}
				}
				for (dist_start, dist_end, extra_bits, code) in &DIST_TO_CODE {
					if dist < dist_end {
						self.write_bits(*code, 5);
						self.write_bits(dist - dist_start, *extra_bits);
						break;
					}
				}
			}
		};
	}

	fn new_fixed_codes_block(&mut self, is_final: bool) {
		if self.in_block {
			self.write_huffman(256); // end of block
		}
		self.in_block = true;
		self.write_bits(if is_final {1} else {0}, 1);
		self.write_bits(1, 1);
		self.write_bits(0, 1);
		self.tree = huffman::calc_codes(&huffman::FIXED_CODES);
	}
}

impl<'a, T: Write> Drop for DeflateWriter<'a, T> {
	fn drop(&mut self) {
		self.write_huffman(256); // end of block
		if self.curr_full_bits > 0 {
			self.out.write_all(&[(self.curr_bytes & 0xFF) as u8]).unwrap();
		}
	}
}

const LEN_TO_CODE: [(u32, u32, u8, u32); 29] = [
// (len start, len end, extra bits, code)
(3  , 4  , 0, 257),
(4  , 5  , 0, 258),
(5  , 6  , 0, 259),
(6  , 7  , 0, 260),
(7  , 8  , 0, 261),
(8  , 9  , 0, 262),
(9  , 10 , 0, 263),
(10 , 11 , 0, 264),
(11 , 13 , 1, 265),
(13 , 15 , 1, 266),
(15 , 17 , 1, 267),
(17 , 19 , 1, 268),
(19 , 23 , 2, 269),
(23 , 27 , 2, 270),
(27 , 31 , 2, 271),
(31 , 35 , 2, 272),
(35 , 43 , 3, 273),
(43 , 51 , 3, 274),
(51 , 59 , 3, 275),
(59 , 67 , 3, 276),
(67 , 83 , 4, 277),
(83 , 99 , 4, 278),
(99 , 115, 4, 279),
(115, 131, 4, 280),
(131, 163, 5, 281),
(163, 195, 5, 282),
(195, 227, 5, 283),
(227, 258, 5, 284),
(258, 259, 0, 285)
];

const DIST_TO_CODE: [(u32, u32, u8, u32); 30] = [
// (dist start, dist end, extra bits, bit reversed code)
(1    , 2    , 0 , 0 ),
(2    , 3    , 0 , 16),
(3    , 4    , 0 , 8 ),
(4    , 5    , 0 , 24),
(5    , 7    , 1 , 4 ),
(7    , 9    , 1 , 20),
(9    , 13   , 2 , 12),
(13   , 17   , 2 , 28),
(17   , 25   , 3 , 2 ),
(25   , 33   , 3 , 18),
(33   , 49   , 4 , 10),
(49   , 65   , 4 , 26),
(65   , 97   , 5 , 6 ),
(97   , 129  , 5 , 22),
(129  , 193  , 6 , 14),
(193  , 257  , 6 , 30),
(257  , 385  , 7 , 1 ),
(385  , 513  , 7 , 17),
(513  , 769  , 8 , 9 ),
(769  , 1025 , 8 , 25),
(1025 , 1537 , 9 , 5 ),
(1537 , 2049 , 9 , 21),
(2049 , 3073 , 10, 13),
(3073 , 4097 , 10, 29),
(4097 , 6145 , 11, 3 ),
(6145 , 8193 , 11, 19),
(8193 , 12289, 12, 11),
(12289, 16385, 12, 27),
(16385, 24577, 13, 7 ),
(24577, 32769, 13, 23)
];
