use std::io::Write;

use crate::huffman;

enum Token {
	Literal(u32),
	// Repeat (u32, u32),
}

pub fn deflate<T: Write>(file: &[u8], out: &mut T) -> std::io::Result<()> {
	let tokens: Vec<Token> = file.iter().map(|x| Token::Literal(*x as u32)).collect();
	let mut writer = DeflateWriter::new(out);

	writer.new_fixed_codes_block(true)?;
	for t in tokens {
		writer.write(&t)?;
	}
	Ok(())
}

struct DeflateWriter<'a, T: Write> {
	out: &'a mut T,
	curr_byte: u8,
	curr_full_bits: u8,
	tree: huffman::Tree,
	in_block: bool,
}

impl<'a, T: Write> DeflateWriter<'a, T> {
	fn new(out: &'a mut T) -> DeflateWriter<'a, T> {
		DeflateWriter {
			out,
			curr_byte: 0,
			curr_full_bits: 0,
			tree: vec![],
			in_block: false,
		}
	}

	fn write_bit(&mut self, bit: u8) -> std::io::Result<()> {
		if self.curr_full_bits == 8 {
			self.out.write_all(&[self.curr_byte])?;
			self.curr_byte = 0;
			self.curr_full_bits = 0;
		}
		self.curr_byte |= bit << self.curr_full_bits;
		self.curr_full_bits += 1;
		Ok(())
	}

	fn write_huffman(&mut self, x: &u32) -> std::io::Result<()> {
		let code = self.tree[*x as usize];
		for bit_num in 0..code.length {
			self.write_bit(((code.code >> bit_num) & 1) as u8)?;
		}
		Ok(())
	}

	fn write(&mut self, token: &Token) -> std::io::Result<()> {
		match token {
			Token::Literal(value) => self.write_huffman(value)?,
		};
		Ok(())
	}

	fn new_fixed_codes_block(&mut self, is_final: bool) -> std::io::Result<()> {
		if self.in_block {
			self.write_huffman(&256)?; // end of block
		}
		self.write_bit(if is_final {1} else {0})?;
		self.write_bit(1)?;
		self.write_bit(0)?;
		self.tree = huffman::calc_codes(&huffman::FIXED_CODES);
		Ok(())
	}
}

impl<'a, T: Write> Drop for DeflateWriter<'a, T> {
	fn drop(&mut self) {
		self.write_huffman(&256).unwrap(); // end of block
		self.out.write_all(&[self.curr_byte]).unwrap();
	}
}
