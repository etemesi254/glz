use std::cmp::min;

use crate::constants::{HASH_CHAINS_MINIMAL_MATCH, MIN_MATCH};
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
    }

    pub(crate) fn advance(&mut self, window: &[u8], start: usize, steps: usize)
    {
        for i in 1..steps
        {
            let hash_entry = hash_chains_hash(window, start + i, 5, self.hash_log);

            self.add_entry((start + i) as u32, hash_entry);
        }
    }
    /// Add a new entry to a cache table
    ///
    /// # Arguments
    /// -  offset: New number to add to the hash chain
    /// -  position: The node in the hash chain
    pub fn add_entry(&mut self, offset: u32, position: usize)
    {
        self.entries[position] = self.entries[position].saturating_add(1);
        self.table[position].push(offset);
    }
    pub fn insert_and_get_longest_match_greedy(
        &mut self, source: &[u8], window_pos: usize, num_literals: usize
    ) -> (usize, usize)
    {
        let hash = hash_chains_hash(source, window_pos, HASH_CHAINS_MINIMAL_MATCH, self.hash_log);

        let previous_entries = &mut self.entries[hash];

        if *previous_entries == 0
        {
            *previous_entries = 1;
            self.table[hash].push(window_pos as u32);
            return (0, 0);
        }
        // We have a previous occurrence
        // Go find your match
        self.find_match(source, hash, window_pos, num_literals)
    }
    fn find_match(
        &mut self, source: &[u8], hash: usize, window_position: usize, num_literals: usize
    ) -> (usize, usize)
    {
        let list = self.table.get(hash).unwrap();
        let searches_to_perform = min(list.len(), self.maximum_depth);

        let mut maximum_match_length = MIN_MATCH;
        let mut max_offset = 0_usize;
        let mut cost = 0;

        // last elements contains most recent match, so we run from that side
        // to ensure we search the nearest offset first
        for previous_offset in list.iter().rev().take(searches_to_perform)
        {
            let offset = *previous_offset as usize;

            if offset == window_position
            {
                continue;
            }
            let previous_match_start = &source[window_position..];

            let curr_match = count(previous_match_start, &source[offset..]);
            let curr_offset = window_position - offset;
            let new_cost = estimate_header_cost(num_literals, curr_match, curr_offset) as usize;
            // new cost
            let nc = (curr_match * 8).saturating_sub(new_cost);
            // old cost
            let oc = (maximum_match_length * 8_usize).saturating_sub(cost);

            if nc >= oc
            {
                maximum_match_length = curr_match;
                max_offset = offset;
                cost = new_cost;
            }
            else if nc == oc
                && nc != 0
                && curr_match == maximum_match_length
                && offset > max_offset
            {
                max_offset = offset;
            }
        }
        self.table[hash].push(window_position as u32);

        (maximum_match_length, max_offset)
    }
}

pub fn estimate_header_cost(literals: usize, ml: usize, offset: usize) -> u32
{
    let mut l_cost = 0;
    let mut off_cost = 1;
    let mut ml_cost = 0;

    if literals > 7
    {
        l_cost += 1;
        l_cost += 2 * u32::from(literals > 0x80);
    }

    let token_match = usize::from(MIN_MATCH) + 7;

    if ml > token_match
    {
        ml_cost += 1;
        ml_cost += 2 * u32::from(ml > 0x80);
    }
    off_cost += 2 * u32::from(offset > 0x80);

    l_cost + ml_cost + off_cost
}
