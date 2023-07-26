use crate::compress::EncodeSequence;
use crate::constants::{BLOCK_SIZE, GLZ_MIN_MATCH, WINDOW_SIZE};
use crate::utils::{compress_sequence, count, prefetch, v_hash};

const HASH_FOUR_LOG_SIZE: usize = 17;
const HASH_THREE_LOG_SIZE: usize = 15;
const FIRST_BYTE_OFFSET: u32 = 24;

const HASH_FOUR_SIZE: usize = 1 << HASH_FOUR_LOG_SIZE;

#[inline(never)]
#[allow(clippy::too_many_lines, unused_assignments)]
pub fn compress_block(src: &[u8], dest: &mut [u8], table: &mut HcMatchFinder) -> usize
{
    let mut window_start = 0;
    let mut literals_before_match = 0;
    let skip_literals = 1;
    let mut out_position = 0;
    let mut compressed_bytes = 0;

    let mut sequence = EncodeSequence::default();

    'match_loop: loop
    {
        // main match finder loop
        'inner_loop: loop
        {
            if window_start + skip_literals + WINDOW_SIZE > src.len()
            {
                // close to input end
                break 'match_loop;
            }

            if table.longest_four_match(src, window_start, literals_before_match, &mut sequence)
            {
                sequence.ll = literals_before_match;
                break 'inner_loop;
            }

            window_start += skip_literals;
            literals_before_match += skip_literals;
        }
        compressed_bytes += sequence.ll + sequence.ml;
        compress_sequence::<false>(src, dest, &mut out_position, &sequence);

        table.advance_four_match(src, window_start, sequence.ml);
        literals_before_match = 0;

        window_start += sequence.ml as usize;

        sequence.ml = 0;

        if window_start + WINDOW_SIZE + skip_literals > src.len()
        {
            // close to input end
            break 'match_loop;
        }
    }
    {
        assert_eq!(sequence.ml, 0);

        sequence.ll = src
            .len()
            .wrapping_sub(window_start)
            .wrapping_add(literals_before_match);

        sequence.ol = 10;
        sequence.start = src.len() - (sequence.ll as usize);
        sequence.ml = GLZ_MIN_MATCH;
        compress_sequence::<true>(src, dest, &mut out_position, &sequence);

        compressed_bytes += sequence.ll as usize;
    }
    table.reset();
    assert_eq!(compressed_bytes, src.len());

    return out_position;
}

pub struct HcMatchFinder
{
    next_hash:    [usize; 2],
    hc_tab:       [u32; 1 << HASH_FOUR_LOG_SIZE],
    hb_tab:       [u32; 1 << HASH_THREE_LOG_SIZE],
    next_tab:     Box<[u32; BLOCK_SIZE]>,
    search_depth: i32,
    min_length:   usize,
    nice_length:  usize
}

impl HcMatchFinder
{
    /// create a new match finder
    pub fn new(
        buf_size: usize, search_depth: i32, min_length: usize, nice_length: usize
    ) -> HcMatchFinder
    {
        let n_tab = vec![0; buf_size].into_boxed_slice();
        //debug_assert!(min_length == 4);
        HcMatchFinder {
            next_hash: [0, 0],
            hc_tab: [0; 1 << HASH_FOUR_LOG_SIZE],
            hb_tab: [0; 1 << HASH_THREE_LOG_SIZE],
            next_tab: n_tab.try_into().expect("Uh oh, fix values bro :)"),
            search_depth,
            nice_length,
            min_length
        }
    }

    pub fn reset(&mut self)
    {
        self.hc_tab.fill(0);
        self.hb_tab.fill(0);
        self.next_hash.fill(0);
    }
    #[inline(always)]
    pub fn longest_four_match(
        &mut self, bytes: &[u8], start: usize, literal_length: usize, sequence: &mut EncodeSequence
    ) -> bool
    {
        let curr_start = &bytes[start..];
        // store the current first byte in the hash, we use this to
        // determine if a match is either a true mach or a hash collision
        // in the bottom
        let curr_match_byte = usize::from(curr_start[0]);
        let curr_byte = u32::from(curr_start[0]) << FIRST_BYTE_OFFSET;

        let next_window = unsafe { bytes.as_ptr().add(start + 1) };
        /* Get the precomputed hash codes */
        let hash = self.next_hash[1];
        /* From the hash buckets, get the first node of each linked list. */
        let mut cur_offset = self.hc_tab[hash % HASH_FOUR_SIZE] as usize;

        self.hc_tab[hash % HASH_FOUR_SIZE] = curr_byte | (start as u32);
        self.next_tab[start % BLOCK_SIZE] = cur_offset as u32;

        //  compute the next hash codes
        let n_hash4 = unsafe { v_hash::<4>(next_window, HASH_FOUR_LOG_SIZE) };
        prefetch(self.hc_tab.as_ptr(), n_hash4);
        prefetch(bytes.as_ptr(), cur_offset);

        self.next_hash[1] = n_hash4 as usize;
        let mut match_found = false;

        if cur_offset != 0
        {
            // top byte is usually first match offset, so remove it
            let mut first_match_byte = cur_offset >> FIRST_BYTE_OFFSET;

            cur_offset &= (1 << FIRST_BYTE_OFFSET) - 1;

            let mut depth = self.search_depth;

            'outer: loop
            {
                if cur_offset == 0 || depth <= 0
                {
                    return match_found;
                }
                'inner: loop
                {
                    depth -= 1;

                    // compare first byte usually stored in hc_tab and next tab for
                    // the offset
                    if first_match_byte == curr_match_byte
                    {
                        // found a possible match, break to see how
                        // long it is
                        // this calls into extend
                        break 'inner;
                    }

                    cur_offset = self.next_tab[cur_offset % BLOCK_SIZE] as usize;
                    first_match_byte = cur_offset >> FIRST_BYTE_OFFSET;
                    cur_offset &= (1 << FIRST_BYTE_OFFSET) - 1;

                    if depth <= 0 || cur_offset == 0
                    {
                        // no match found
                        // go and try the other tab
                        break 'outer;
                    }
                }
                if match_found
                {
                    unsafe {
                        // we have a previous match, check if current match length will go past
                        // the previous match length by looking at the byte in current length plus 1
                        // if they match, then this has the potential to beat the previous ML
                        let prev_match_end = bytes.get_unchecked(cur_offset + sequence.ml);
                        // N.B: This may read +1 byte past curr_start, but that is okay
                        let curr_match_end = curr_start.get_unchecked(sequence.ml);

                        if prev_match_end != curr_match_end
                        {
                            // go to next node
                            cur_offset = self.next_tab[cur_offset % BLOCK_SIZE] as usize;
                            first_match_byte = cur_offset >> FIRST_BYTE_OFFSET;
                            cur_offset &= (1 << FIRST_BYTE_OFFSET) - 1;

                            depth -= 1;
                            continue;
                        }
                    }
                }
                // extend
                let new_match_length =
                    unsafe { count(bytes.get_unchecked(cur_offset..), curr_start) };

                let diff = start - cur_offset;

                if new_match_length >= self.min_length && new_match_length > sequence.ml && diff > 3
                {
                    sequence.ml = new_match_length;
                    sequence.ol = diff;
                    sequence.start = start - literal_length;

                    match_found = true;

                    if new_match_length > self.nice_length
                    {
                        return true;
                    }
                }
                // go to next node
                cur_offset = self.next_tab[cur_offset % BLOCK_SIZE] as usize;
                first_match_byte = cur_offset >> FIRST_BYTE_OFFSET;
                cur_offset &= (1 << FIRST_BYTE_OFFSET) - 1;

                depth -= 1;
            }
        }
        return match_found;
    }

    #[inline(always)]
    pub fn advance_four_match(&mut self, window_start: &[u8], mut start: usize, mut length: usize)
    {
        if (start + length + 100) < window_start.len()
        {
            let mut hash4 = self.next_hash[1];
            loop
            {
                unsafe {
                    let next_window = window_start.as_ptr().add(start + 1);

                    let curr_byte =
                        u32::from(*window_start.get_unchecked(start)) << FIRST_BYTE_OFFSET;

                    self.next_tab[start % BLOCK_SIZE] = self.hc_tab[hash4 % HASH_FOUR_SIZE];
                    self.hc_tab[hash4 % HASH_FOUR_SIZE] = curr_byte | (start as u32);
                    start += 1;
                    //  compute the next hash codes
                    hash4 = v_hash::<4>(next_window, HASH_FOUR_LOG_SIZE) as usize;
                    length -= 1;

                    if length <= 0
                    {
                        break;
                    }
                }
            }
            self.next_hash[1] = hash4 as usize;
            prefetch(self.hc_tab.as_ptr(), hash4);
        }
    }
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
