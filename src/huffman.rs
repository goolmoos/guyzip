use std::collections::VecDeque;

#[derive(Copy, Clone)]
pub struct HuffmanCode {
	pub code: u32,
	pub length: u8,
}

pub type Tree = Vec<HuffmanCode>;

// deflate, Compression with fixed Huffman codes:
pub const LITERAL_FIXED_CODES: [u8; 288] = [
8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,
9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
8,8,8,8,8,8,8,8
];
pub const DISTANCE_FIXED_CODES: [u8; 32] = [
5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5
];

pub fn calc_codes(lengths: &[u8]) -> Tree {
	// codes generated are meant to be read LSB to MSB

	let mut bl_count: [u32; 16] = [0; 16]; // 15 is max bit length
	for bl in lengths {
		bl_count[*bl as usize] += 1;
	}

	let mut next_code: [u32; 16] = [0; 16]; // start of range for codes with length [index]
	let mut code: u32 = 0;
	bl_count[0] = 0;
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

pub fn gen_lengths(weights: &[u64], l: u8, out: &mut[u8]) {
	assert_eq!(weights.len(), out.len());
	out.iter_mut().for_each(|x| *x = 0);

	// finds optimal huffman tree with length bound l (including) and given weights. stores code lens in out.
	// based on the algorith presented in https://www.ics.uci.edu/~dan/pubs/LenLimHuff.pdf
	#[derive(Clone)]
	struct Package {
		weight: u64,
		contents: Vec<usize>,
	}
	let mut levels_left = l;
	let mut curr_packages: VecDeque<Package> = VecDeque::new();
	let mut new_level: Vec<Package> = weights.iter()
		.enumerate()
		.filter(|(_i, w)| **w != 0)
		.map(|(i, w)|
			Package {
				weight: *w,
				contents: vec![i],
			})
		.collect();
	new_level.sort_by_key(|p| std::cmp::Reverse(p.weight));
	let new_level = VecDeque::from(new_level);
	let mut x = (new_level.len() - 1) << l;
	while x > 0 {
		if levels_left > 0 {
			curr_packages = merge(curr_packages, new_level.clone());
			levels_left -= 1;
		}
		if x & 1 == 1 {
			curr_packages.pop_back().unwrap().contents.iter().for_each(|i| out[*i] += 1);
		}
		curr_packages = package(curr_packages);
		x >>= 1;
	}

	fn merge(mut a: VecDeque<Package>, mut b: VecDeque<Package>) -> VecDeque<Package> {
		let mut res = VecDeque::new();
		res.reserve(a.len() + b.len());
		while a.len() > 0 && b.len() > 0 {
			if a.back().unwrap().weight < b.back().unwrap().weight {
				res.push_front(a.pop_back().unwrap());
			} else {
				res.push_front(b.pop_back().unwrap());
			}
		}
		while a.len() > 0 {
			res.push_front(a.pop_back().unwrap());
		}
		while b.len() > 0 {
			res.push_front(b.pop_back().unwrap());
		}
		res
	}

	fn package(mut v: VecDeque<Package>) -> VecDeque<Package> {
		let mut res = VecDeque::new();
		res.reserve(v.len() / 2);
		for _ in 0..v.len() / 2 {
			let mut p0 = v.pop_back().unwrap();
			let mut p1 = v.pop_back().unwrap();
			p0.contents.append(&mut p1.contents);
			res.push_front(Package {
				weight: p0.weight + p1.weight,
				contents: p0.contents,
			})
		}
		res
	}
}
