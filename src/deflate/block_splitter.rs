use super::{Token, LEN_TO_CODE, DIST_TO_CODE};
use crate::huffman;

pub enum Block<'a> {
	FixedCodes { tokens: &'a[Token] },
	DynamicCodes { tokens: &'a[Token], literal_code_lens: [u8; 286], distance_code_lens: [u8; 30] },
}

pub fn block_split(tokens: &[Token]) -> Vec<Block> {
	const BLOCK_SIZE: usize = 8192;

	let mut blocks = vec![];

	let mut literal_code_lens = [0; 286];
	let mut distance_code_lens = [0; 30];

	for i in (0..tokens.len()).step_by(BLOCK_SIZE) {
		let start = i;
		let end = if i + BLOCK_SIZE < tokens.len() {i + BLOCK_SIZE} else {tokens.len()};
		let tokens = &tokens[start..end];

		let mut counter = FreqCounter::new();
		for t in tokens {
			counter.count(t);
		}
		huffman::gen_lengths(&counter.literal_count, 15, &mut literal_code_lens);
		huffman::gen_lengths(&counter.distance_count, 15, &mut distance_code_lens);
		blocks.push(Block::DynamicCodes {
			tokens,
			literal_code_lens,
			distance_code_lens,
		});
	}

	blocks
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
				for (_len_start, len_end, _extra_bits, code) in &LEN_TO_CODE {
					if len < len_end {
						self.literal_count[*code as usize] += 1;
						break;
					}
				}
				for (_dist_start, dist_end, _extra_bits, code) in &DIST_TO_CODE {
					if dist < dist_end {
						self.distance_count[*code as usize] += 1;
						break;
					}
				}
			}
		};
	}
}
