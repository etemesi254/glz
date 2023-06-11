use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use crate::compress::hash_chains::compress_block;
use crate::compress::hash_chains::hash_chains::HashChains;
use crate::constants::{
    COMPRESSION_LEVEL, DEPTH_STRIDE, HASH_CHAINS_BUCKET_LOG, MEM_SIZE, SLOP_BYTES
};

mod hash_chains;

#[derive(Copy, Clone, Default, Debug)]
pub struct EncodeSequence
{
    start: usize,
    ll:    usize,
    ml:    usize,
    ol:    usize,
    cost:  usize
}

pub fn compress(input_file: String, output_file: String)
{
    const BLOCK_SIZE: usize = 256 * (1 << 10); //1 * (1 << 20);

    let mut table = HashChains::new(
        1 << HASH_CHAINS_BUCKET_LOG,
        HASH_CHAINS_BUCKET_LOG,
        COMPRESSION_LEVEL * DEPTH_STRIDE
    );

    let p = Path::new(&input_file);

    // allocate and add slack bytes, so that we don't panic in simd_decode
    let mut max_in = Vec::with_capacity(MEM_SIZE + SLOP_BYTES);
    max_in.resize(MEM_SIZE, 0);

    let mut max_out = Vec::with_capacity(MEM_SIZE + SLOP_BYTES);
    max_out.resize(MEM_SIZE, 0);

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

        //dbg!(bytes_compressed);
        max_out[0..4].copy_from_slice(&(bytes_compressed as u32).to_le_bytes());

        //panic!();
        table.clear();
        total_bytes += bytes_compressed;
        out_fd.write_all(&max_out[..bytes_compressed + 4]).unwrap();
        //dbg!(ratio, block_ratio);
        // panic!();
    }

    let end = Instant::now();

    println!("time: {:?}", end - start);
    println!("Compressed {} to {}", total_bytes_read, total_bytes);
}
