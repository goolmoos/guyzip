use super::{Token, deflate_code_of_len, deflate_code_of_dist};
use crate::huffman;
use crate::deflate;

pub enum Block<'a> {
	FixedCodes { tokens: &'a[Token] },
	DynamicCodes { tokens: &'a[Token], literal_code_lens: [u8; 286], distance_code_lens: [u8; 30] },
}

struct BlockInProgress {
	start: usize,
	end: usize,
	freqs: FreqCounter,
	literal_code_lens: [u8; 286],
	distance_code_lens: [u8; 30],
	cost: u64,
	is_dynamic: bool,
}

pub fn block_split(tokens: &[Token]) -> Vec<Block> {
	const BLOCK_SIZE: usize = 1024;

	let mut blocks = vec![];

	let mut curr_block = None;
	for i in (0..tokens.len()).step_by(BLOCK_SIZE) {
		let start = i;
		let end = if i + BLOCK_SIZE < tokens.len() {i + BLOCK_SIZE} else {tokens.len()};

		let next_block = BlockInProgress::new(start, end, tokens);
		match curr_block {
			None => curr_block = Some(next_block),
			Some(b) => {
				let combined_block = BlockInProgress::merge(&b, &next_block);
				if combined_block.cost < b.cost + next_block.cost {
					curr_block = Some(combined_block);
				} else {
					blocks.push(build_block(b, tokens));
					curr_block = Some(next_block);
				}
			}
		}
	}
	if let Some(b) = curr_block {
		blocks.push(build_block(b, tokens));
	}
	blocks
}

impl BlockInProgress {
	fn new(start: usize, end: usize, all_tokens: &[Token]) -> BlockInProgress {
		let tokens = &all_tokens[start..end];
		let mut counter = FreqCounter::new();
		for t in tokens {
			counter.count(t);
		}
		let mut literal_code_lens = [0; 286];
		let mut distance_code_lens = [0; 30];
		huffman::gen_lengths(&counter.literal_count, 15, &mut literal_code_lens);
		huffman::gen_lengths(&counter.distance_count, 15, &mut distance_code_lens);

		let dynamic_cost = block_cost(&counter, &literal_code_lens, &distance_code_lens) + dynamic_header_cost(&literal_code_lens, &distance_code_lens);
		let fixed_cost = 3 + block_cost(&counter, &huffman::LITERAL_FIXED_CODES, &huffman::DISTANCE_FIXED_CODES);

		let (cost, is_dynamic) = if dynamic_cost < fixed_cost {
			(dynamic_cost, true)
		} else {
			(fixed_cost, false)
		};

		BlockInProgress {
			start,
			end,
			freqs: counter,
			literal_code_lens,
			distance_code_lens,
			cost,
			is_dynamic,
		}
	}

	fn merge(b1: &BlockInProgress, b2: &BlockInProgress) -> BlockInProgress {
		let mut literal_count = [0; 286];
		let mut distance_count = [0; 30];
		for i in 0..286 {
			literal_count[i] = b1.freqs.literal_count[i] + b2.freqs.literal_count[i];
		}
		for i in 0..30 {
			distance_count[i] = b1.freqs.distance_count[i] + b2.freqs.distance_count[i];
		}
		literal_count[256] = 1; // only one end of block symbol
		let freqs = FreqCounter { literal_count, distance_count };

		let mut literal_code_lens = [0; 286];
		let mut distance_code_lens = [0; 30];
		huffman::gen_lengths(&freqs.literal_count, 15, &mut literal_code_lens);
		huffman::gen_lengths(&freqs.distance_count, 15, &mut distance_code_lens);

		let dynamic_cost = block_cost(&freqs, &literal_code_lens, &distance_code_lens) + dynamic_header_cost(&literal_code_lens, &distance_code_lens);
		let fixed_cost = 3 + block_cost(&freqs, &huffman::LITERAL_FIXED_CODES, &huffman::DISTANCE_FIXED_CODES);

		let (cost, is_dynamic) = if dynamic_cost < fixed_cost {
			(dynamic_cost, true)
		} else {
			(fixed_cost, false)
		};

		BlockInProgress {
			start: b1.start,
			end: b2.end,
			freqs,
			literal_code_lens,
			distance_code_lens,
			cost,
			is_dynamic,
		}
	}
}

fn build_block(block: BlockInProgress, all_tokens: &[Token]) -> Block {
	if block.is_dynamic {
		Block::DynamicCodes {
			tokens: &all_tokens[block.start..block.end],
			literal_code_lens: block.literal_code_lens,
			distance_code_lens: block.distance_code_lens,
		}
	} else {
		Block::FixedCodes {
			tokens: &all_tokens[block.start..block.end],
		}
	}
}

fn dynamic_header_cost(literal_code_lens: &[u8], distance_code_lens: &[u8]) -> u64 {
	let mut res: u64 = 0;
	let incrementor = |_bits: u32, len: u8| res += len as u64;
	deflate::create_dynamic_block_header(&literal_code_lens, &distance_code_lens, incrementor);
	res
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
