use super::{Token, deflate_code_of_len, deflate_code_of_dist};
use crate::huffman;
use crate::deflate;

pub enum Block<'a> {
	FixedCodes { tokens: &'a[Token] },
	DynamicCodes { tokens: &'a[Token], literal_code_lens: [u8; 286], distance_code_lens: [u8; 30] },
}

pub fn block_split(tokens: &[Token]) -> Vec<Block> {
	const BLOCK_SIZE: usize = 8192;

	let mut blocks = vec![];

	for i in (0..tokens.len()).step_by(BLOCK_SIZE) {
		let start = i;
		let end = if i + BLOCK_SIZE < tokens.len() {i + BLOCK_SIZE} else {tokens.len()};
		let tokens = &tokens[start..end];

		blocks.push(build_block(tokens));
	}

	blocks
}

fn build_block(tokens: &[Token]) -> Block {
	let mut counter = FreqCounter::new();
	for t in tokens {
		counter.count(t);
	}
	let mut literal_code_lens = [0; 286];
	let mut distance_code_lens = [0; 30];
	huffman::gen_lengths(&counter.literal_count, 15, &mut literal_code_lens);
	huffman::gen_lengths(&counter.distance_count, 15, &mut distance_code_lens);

	let mut dynamic_cost_header: u64 = 0;
	let incrementor = |_bits: u32, len: u8| dynamic_cost_header += len as u64;
	deflate::create_dynamic_block_header(&literal_code_lens, &distance_code_lens, incrementor);

	let dynamic_cost_body = block_cost(&counter, &literal_code_lens, &distance_code_lens);
	let fixed_cost_header = 3;
	let fixed_cost_body = block_cost(&counter, &huffman::LITERAL_FIXED_CODES, &huffman::DISTANCE_FIXED_CODES);

	if dynamic_cost_header + dynamic_cost_body < fixed_cost_header + fixed_cost_body {
		Block::DynamicCodes {
			tokens,
			literal_code_lens,
			distance_code_lens,
		}
	} else {
		Block::FixedCodes {
			tokens,
		}
	}
}

fn block_cost(counter: &FreqCounter, literal_code_lens: &[u8], distance_code_lens: &[u8]) -> u64 {
	let mut total = 0;
	total += counter.literal_count.iter().zip(literal_code_lens.iter()).map(|(a, b)| a * (*b as u64)).sum::<u64>();
	total += counter.distance_count.iter().zip(distance_code_lens.iter()).map(|(a, b)| a * (*b as u64)).sum::<u64>();
	total
}

struct FreqCounter {
	literal_count: [u64; 286],
	distance_count: [u64; 30],
}

impl FreqCounter {
	fn new() -> FreqCounter {
		let mut res = FreqCounter {
			literal_count: [0; 286],
			distance_count: [0; 30],
		};
		res.literal_count[256] = 1; // end of block
		res
	}

	fn count(&mut self, token: &Token) {
		match token {
			Token::Literal(value) => self.literal_count[*value as usize] += 1,
			Token::Repeat(len, dist) => {
				let (_offset, _extra_bits, code) = deflate_code_of_len(*len);
				self.literal_count[code as usize] += 1;
				let (_offset, _extra_bits, code) = deflate_code_of_dist(*dist);
				self.distance_count[code as usize] += 1;
			}
		}
	}
}
