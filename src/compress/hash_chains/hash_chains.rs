use std::cmp::min;

use crate::compress::EncodeSequence;
use crate::constants::{GLZ_MIN_MATCH, HASH_CHAINS_MINIMAL_MATCH};
use crate::utils::{count, hash_chains_hash};

pub struct HashChains
{
    hash_log:      usize,
    maximum_depth: usize,
    entries:       Vec<u16>,
    // I'm sorry to the cache gods...
    // The newest elements are found in the end of the vec, older
    // entries in the start
    table:         Vec<Vec<u32>>
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
            let new_cost = estimate_header_cost(num_literals, curr_match, curr_offset) as usize;
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

                // good enough match
                if nc > 100
                {
                    break;
                }
            }
        }
        self.table[hash].push(window_position as u32);
        valid_seq
    }
}

pub fn estimate_header_cost(literals: usize, ml: usize, offset: usize) -> u32
{
    let mut l_cost = 0;
    let mut off_cost = 1;
    let mut ml_cost = 0;

    if literals > 7
    {
        l_cost += 1 + u32::from((usize::BITS - literals.leading_zeros()) / 8);
    }

    let token_match = usize::from(GLZ_MIN_MATCH) + 7;

    if ml > token_match
    {
        ml_cost += 1 + u32::from((usize::BITS - ml.leading_zeros()) >> 3);
    }
    off_cost += u32::from((usize::BITS - offset.leading_zeros()) >> 3);

    1 + l_cost + ml_cost + off_cost
}
