use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use crate::compress::hash_chains::compress_block;
use crate::compress::hash_chains::hash_chains::HashChains;
use crate::constants::{COMPRESSION_LEVEL, DEPTH_STRIDE, HASH_CHAINS_BUCKET_LOG};

mod hash_chains;

#[derive(Copy, Clone, Default, Debug)]
struct EncodeSequence
{
    start: usize,
    ll:    usize,
    ml:    usize,
    ol:    usize
}

pub fn compress(input_file: String, output_file: String)
{
    const BLOCK_SIZE: usize = 10 * (1 << 20);

    let mut table = HashChains::new(
        1 << HASH_CHAINS_BUCKET_LOG,
        HASH_CHAINS_BUCKET_LOG,
        COMPRESSION_LEVEL * DEPTH_STRIDE
    );

    let p = Path::new(&input_file);

    // allocate and add slack bytes, so that we don't panic in simd_decode
    let mut max_in = Vec::with_capacity(116777216 + 160);
    max_in.resize(116777216, 0);

    let mut max_out = Vec::with_capacity(116777216 + 160);
    max_out.resize(116777216, 0);

    let start = Instant::now();
    let mut fd = File::open(p).unwrap();
    let mut out_fd = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&output_file)
        .unwrap();

    loop
    {
        let bytes_read = fd.read(&mut max_in[0..BLOCK_SIZE]).unwrap();
        if bytes_read == 0
        {
            break;
        }
        let bytes_written = compress_block(&max_in[..bytes_read], &mut max_out[4..], &mut table);
        dbg!(bytes_written);
        table.clear();
    }

    let end = Instant::now();

    println!("time: {:?}", end - start);
}
