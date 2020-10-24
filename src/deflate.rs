use std::io::Write;

use crate::huffman;

enum Token {
	Literal(u32),
	// Repeat (u32, u32),
}

pub fn deflate<T: Write>(file: &[u8], out: &mut T) {
	let tokens: Vec<Token> = file.iter().map(|x| Token::Literal(*x as u32)).collect();
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

	fn write_huffman(&mut self, x: &u32){
		let code = self.tree[*x as usize];
		self.write_bits(code.code, code.length);
	}

	fn write(&mut self, token: &Token) {
		match token {
			Token::Literal(value) => self.write_huffman(value),
		};
	}

	fn new_fixed_codes_block(&mut self, is_final: bool) {
		if self.in_block {
			self.write_huffman(&256); // end of block
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
		self.write_huffman(&256); // end of block
		if self.curr_full_bits > 0 {
			self.out.write_all(&[(self.curr_bytes & 0xFF) as u8]).unwrap();
		}
	}
}
