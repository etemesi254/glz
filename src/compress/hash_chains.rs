use std::cmp::min;

use crate::compress::EncodeSequence;
use crate::constants::{GLZ_MIN_MATCH, HASH_CHAINS_MINIMAL_MATCH, SKIP_TRIGGER, WINDOW_SIZE};
use crate::utils::{compress_sequence, count, hash_chains_hash};

#[inline(never)]
#[allow(clippy::too_many_lines, unused_assignments)]
pub fn compress_block(src: &[u8], dest: &mut [u8], table: &mut HashChains) -> usize
{
    let mut window_start = 0;
    let mut literals_before_match = 0;
    let mut sequence = EncodeSequence::default();
    let mut skip_bytes = 0;
    let mut out_position = 0;
    let mut compressed_bytes = 0;

    'match_loop: loop
    {
        // main match finder loop
        'inner_loop: loop
        {
            if window_start + WINDOW_SIZE > src.len()
            {
                // close to input end
                break 'match_loop;
            }

            table.prefetch(&src[window_start + 1..]);

            if table.find_longest_match_greedy(
                src,
                window_start,
                literals_before_match,
                &mut sequence
            )
            {
                break 'inner_loop;
            }

            let skip_literals = 1 + (skip_bytes >> SKIP_TRIGGER);
            skip_bytes += 1;
            literals_before_match += skip_literals;
            window_start += skip_literals;
        }

        compressed_bytes += sequence.ll + sequence.ml;

        compress_sequence::<false>(src, dest, &mut out_position, &sequence);

        literals_before_match = 0;
        skip_bytes = 0;

        window_start += sequence.ml;
        sequence.ml = 0;

        if window_start + WINDOW_SIZE > src.len()
        {
            // close to input end
            break 'match_loop;
        }
    }
    {
        assert_eq!(sequence.ml, 0);
        sequence.ll = (src.len() - window_start) + literals_before_match;
        sequence.ol = 0;
        sequence.start = src.len() - sequence.ll;
        // so write_token works
        sequence.ml = GLZ_MIN_MATCH;

        compressed_bytes += sequence.ll;

        compress_sequence::<true>(src, dest, &mut out_position, &sequence);
    }
    assert_eq!(compressed_bytes, src.len());

    return out_position;
}

pub struct HashChains
{
    hash_log:      usize,
    maximum_depth: usize,
    pub entries:   Vec<u16>,
    // I'm sorry to the cache gods...
    // The newest elements are found in the end of the vec, older
    // entries in the start
    pub table:     Vec<Vec<u32>>
}

impl HashChains
{
    pub fn new(rows: usize, hash_log: usize, maximum_depth: usize) -> Self
    {
        HashChains {
            hash_log,
            entries: vec![0; rows],
            maximum_depth,
            table: vec![Vec::with_capacity(maximum_depth / 2); rows]
        }
    }
    pub fn clear(&mut self)
    {
        self.entries.fill(0);

        self.table.iter_mut().for_each(|x| unsafe {
            x.set_len(0);
        });
    }
    #[inline(always)]
    pub(crate) fn prefetch(&self, src: &[u8])
    {
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse"
        ))]
        {
            unsafe {
                let hash = hash_chains_hash(src, 0, HASH_CHAINS_MINIMAL_MATCH, self.hash_log);
                // SAFETY: We are assured that we are running in a processor capable of executing
                // this instruction
                use core::arch::x86_64::_mm_prefetch;
                _mm_prefetch::<3>(self.table.as_ptr().add(hash).cast::<i8>());
            }
        }
    }

    pub fn find_longest_match_greedy(
        &mut self, source: &[u8], window_pos: usize, num_literals: usize,
        sequence: &mut EncodeSequence
    ) -> bool
    {
        debug_assert!(window_pos >= num_literals);
        sequence.ll = 0;
        sequence.ml = 0;
        sequence.ol = 0;
        sequence.cost = 0;
        sequence.start = window_pos - num_literals;

        let hash = hash_chains_hash(source, window_pos, HASH_CHAINS_MINIMAL_MATCH, self.hash_log);

        let previous_entries = &mut self.entries[hash];

        if *previous_entries == 0
        {
            *previous_entries = 1;
            self.table[hash].push(window_pos as u32);
            return false;
        }
        // We have a previous occurrence
        // Go find your match
        self.find_match(source, hash, window_pos, num_literals, sequence)
    }
    fn find_match(
        &mut self, source: &[u8], hash: usize, window_position: usize, num_literals: usize,
        seq: &mut EncodeSequence
    ) -> bool
    {
        let list = self.table.get(hash).unwrap();
        let searches_to_perform = min(list.len(), self.maximum_depth);

        let mut l_cost = 1;
        l_cost += usize::from(num_literals > 7)
            + ((usize::BITS - num_literals.leading_zeros()) / 8) as usize;

        let mut valid_seq = false;

        let previous_match_start = &source[window_position..];

        // last elements contains most recent match, so we run from that side
        // to ensure we search the nearest offset first
        for previous_offset in list.iter().rev().take(searches_to_perform)
        {
            let offset = *previous_offset as usize;

            if offset == window_position || offset == 0
            {
                continue;
            }

            let curr_match = count(previous_match_start, &source[offset..]);

            if curr_match < GLZ_MIN_MATCH
            {
                continue;
            }
            let curr_offset = window_position - offset;
            let new_cost = l_cost + estimate_header_cost(curr_match, curr_offset) as usize;
            // new cost
            let nc = curr_match as isize - new_cost as isize;
            // old cost
            let oc = seq.ml as isize - seq.cost as isize;

            // dbg!(
            //     num_literals,
            //     new_cost,
            //     curr_offset,
            //     curr_match,
            //     window_position
            // );
            // eprintln!();

            if nc > oc
            {
                seq.cost = new_cost;
                seq.ll = num_literals;
                seq.ml = curr_match;
                seq.ol = curr_offset;

                valid_seq = true;

                // // good enough match
                // if nc > 100
                // {
                //     break;
                // }
            }
        }
        self.table[hash].push(window_position as u32);
        valid_seq
    }
}

pub fn estimate_header_cost(ml: usize, offset: usize) -> u32
{
    let mut off_cost = 1;
    let mut ml_cost = 0;

    let token_match = usize::from(GLZ_MIN_MATCH) + 7;

    ml_cost += u32::from(ml > token_match) + u32::from((usize::BITS - ml.leading_zeros()) >> 3);
    off_cost += u32::from((usize::BITS - offset.leading_zeros()) >> 3);

    ml_cost + off_cost
}

#[test]
fn compress_decompress_encodemod()
{
    use crate::decompress::decode_encode_mod;

    let mut out = [0; 16];
    let value = 13942;
    compress_encode_mod(value, &mut out, &mut 0);
    let recovered = decode_encode_mod(&mut out);
    assert_eq!(recovered.0, value);
}
