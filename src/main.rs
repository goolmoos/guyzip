use std::path::Path;
use std::fs::File;
use std::io::{Read, Write, BufWriter};

mod crc32;
mod huffman;
mod deflate;

fn main() -> std::io::Result<()> {
	let args: Vec<String> = std::env::args().collect();

	let in_path = Path::new(args.get(1).expect("must supply a file to compress"));
	let out_file_name = format!("{}.gz", in_path.file_name().unwrap().to_str().unwrap());
	let out_path = Path::new(&out_file_name);

	let file: Vec<u8> = File::open(in_path)?.bytes().map(|x| x.unwrap()).collect();
	compress(&file, out_path)?;
	Ok(())
}

fn compress(file: &[u8], out_path: &Path) -> std::io::Result<()> {
	let mut out_file = BufWriter::with_capacity(1 << 20, File::create(out_path)?);
	
	// gzip header
	out_file.write_all(&[0x1F, 0x8B])?; // magic
	out_file.write_all(&[0x08])?; // Compression Method = DEFLATE
	out_file.write_all(&[0x00])?; // Flags - none
	out_file.write_all(&[0x00, 0x00, 0x00, 0x00])?; // Modification Time - none
	out_file.write_all(&[0x00])?; // Extra Flags - None
	out_file.write_all(&[0xFF])?; // OS - unknown

	deflate::deflate(file, &mut out_file);

	let crc32 = crc32::crc32(file);
	out_file.write_all(&crc32.to_le_bytes())?; // CRC32
	let size: u32 = file.len() as u32 & 0xFFFFFFFF;
	out_file.write_all(&size.to_le_bytes())?; // size modulo 2^32

	Ok(())
}
