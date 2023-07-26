use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use crate::compress::hash_chains::{compress_block, HcMatchFinder};
use crate::constants::{BLOCK_SIZE, DEPTH_STRIDE, GLZ_MIN_MATCH, MEM_SIZE, SLOP_BYTES};

mod hash_chains;

#[derive(Copy, Clone, Default, Debug)]
pub struct EncodeSequence
{
    pub start: usize,
    pub ll:    usize,
    pub ml:    usize,
    pub ol:    usize,
    pub cost:  usize
}

pub fn compress(input_file: String, output_file: String)
{
    let mut table = HcMatchFinder::new(BLOCK_SIZE, DEPTH_STRIDE as i32, GLZ_MIN_MATCH, 100);

    let p = Path::new(&input_file);

    // allocate and add slack bytes, so that we don't panic in simd_decode
    let mut max_in = Vec::with_capacity(MEM_SIZE + SLOP_BYTES);
    max_in.resize(MEM_SIZE, 0);

    let mut max_out = Vec::with_capacity(MEM_SIZE + SLOP_BYTES);
    max_out.resize(MEM_SIZE, 0);

    // let mut temp = Vec::with_capacity(MEM_SIZE + SLOP_BYTES);
    // temp.resize(MEM_SIZE, 0);

    let start = Instant::now();
    let mut fd = File::open(p).unwrap();
    let mut out_fd = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&output_file)
        .unwrap();

    let mut total_bytes = 0;
    let mut total_bytes_read = 0;
    loop
    {
        let bytes_read = fd.read(&mut max_in[0..BLOCK_SIZE]).unwrap();
        total_bytes_read += bytes_read;

        if bytes_read == 0
        {
            break;
        }

        let bytes_compressed = compress_block(&max_in[..bytes_read], &mut max_out[4..], &mut table);

        max_out[0..4].copy_from_slice(&(bytes_compressed as u32).to_le_bytes());

        table.reset();
        total_bytes += bytes_compressed;
        out_fd.write_all(&max_out[..bytes_compressed + 4]).unwrap();
    }

    let end = Instant::now();

    println!(
        "Compressed {} to {} in {:?}",
        total_bytes_read,
        total_bytes,
        end - start
    );
}
