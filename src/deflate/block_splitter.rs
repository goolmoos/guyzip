use super::{Token, LEN_TO_CODE, DIST_TO_CODE};
use crate::huffman;

pub enum Block<'a> {
	FixedCodes { tokens: &'a[Token] },
	DynamicCodes { tokens: &'a[Token], literal_code_lens: [u8; 286], distance_code_lens: [u8; 30] },
}

pub fn block_split(tokens: &[Token]) -> Vec<Block> {
	let mut counter = FreqCounter::new();
	for t in tokens {
		counter.count(t);
	}

	/*let mut test_code_lens = [0; 5];
	let test_count = [0, 1, 1, 0, 5];
	huffman::gen_lengths(&test_count, 2, &mut test_code_lens);
	for i in 0..5 {
		println!("{}: {}-{}", i, test_code_lens[i], test_count[i]);
	}
	panic!();*/

	let mut literal_code_lens = [0; 286];
	huffman::gen_lengths(&counter.literal_count, 15, &mut literal_code_lens);
	let mut distance_code_lens = [0; 30];
	huffman::gen_lengths(&counter.distance_count, 15, &mut distance_code_lens);

	/*for i in 0..286 {
		println!("{}: {}-{}", i, literal_code_lens[i], counter.literal_count[i]);
	}
	for i in 0..30 {
		println!("{}: {}-{}", i, distance_code_lens[i], counter.distance_count[i]);
	}
	println!("sums: {} {}", counter.literal_count.iter().sum::<u64>(), counter.distance_count.iter().sum::<u64>());
	panic!();*/

	vec![Block::DynamicCodes {
		tokens,
		literal_code_lens,
		distance_code_lens,
	}]
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
