#[derive(Copy, Clone)]
pub struct HuffmanCode {
	pub code: u32,
	pub length: u8,
}

pub type Tree = Vec<HuffmanCode>;

// deflate, Compression with fixed Huffman codes:
pub const FIXED_CODES: [u8; 288] = [
8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,
9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
8,8,8,8,8,8,8,8
];

pub fn calc_codes(lengths: &[u8]) -> Tree {
	// codes generated are meant to be read LSB to MSB

	let mut bl_count: [u32; 16] = [0; 16]; // 15 is max bit length
	for bl in lengths {
		bl_count[*bl as usize] += 1;
	}

	let mut next_code: [u32; 16] = [0; 16]; // start of range for codes with length [index]
	let mut code: u32 = 0;
	for bl in 1..16 {
		code = (code + bl_count[bl - 1]) << 1;
		next_code[bl] = code;
	}

	let mut codes: Vec<HuffmanCode> = vec![]; // assign the codes, MSB to LSB
	for l in lengths {
		codes.push(HuffmanCode {
			code: next_code[*l as usize],
			length: *l,
		});
		next_code[*l as usize] += 1;
	}
	
	// reverse the codes
	for mut huffman_code in &mut codes {
		let mut new_code = 0;
		for _ in 0..huffman_code.length {
			new_code <<= 1;
			new_code |= huffman_code.code & 1;
			huffman_code.code >>= 1;
		}
		huffman_code.code = new_code;
	}

	codes
}
