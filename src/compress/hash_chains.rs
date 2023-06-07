use crate::compress::hash_chains::hash_chains::HashChains;
use crate::compress::EncodeSequence;
use crate::constants::{
    GLZ_MIN_MATCH, LITERAL_BITS, ML_BITS, OFFSET_BIT, TOKEN, UNCOMPRESSED, WINDOW_SIZE
};
use crate::utils::copy_literals;

pub mod hash_chains;

#[inline(always)]
fn write_token(seq: &EncodeSequence) -> u8
{
    debug_assert!(seq.ml >= GLZ_MIN_MATCH);
    let ml_length = (seq.ml - GLZ_MIN_MATCH) as u8;

    let ml_token = if ml_length >= 7 { 7 } else { ml_length };
    let ll_token = if seq.ll >= 7 { 7 } else { seq.ll as u8 };
    let ol_token = (seq.ol & 0b11) as u8;

    let mut out: u8 = 0;

    out |= ol_token << OFFSET_BIT;
    out |= ml_token << ML_BITS;
    out |= ll_token << LITERAL_BITS;

    out
}

fn compress_sequence(src: &[u8], dest: &mut [u8], dest_position: &mut usize, seq: &EncodeSequence)
{
    let token_byte = write_token(seq);
    let mut extra = *seq;
    extra.ll = extra.ll.wrapping_sub(7);
    extra.ml = extra.ml.wrapping_sub(7 + GLZ_MIN_MATCH);
    extra.ol = extra.ol >> 2;
    dest[*dest_position] = token_byte;
    *dest_position += 1;

    // TODO: ADD long literal encoding

    // copy literals
    copy_literals(src, dest, seq.start, *dest_position, seq.ll);
    *dest_position += seq.ll;

    // TODO: Add offset encoding

    // TODO: Add match encoding
}

#[rustfmt::skip]
#[inline(never)]
#[allow(clippy::too_many_lines, unused_assignments)]
pub fn compress_block(
    src: &[u8],
    dest: &mut [u8],
    table: &mut HashChains,
) -> usize
{
    let mut ml = 0;
    let mut offset = 0;
    let mut window_start = 0;
    let mut literals_before_match = 0;
    let mut sequence = EncodeSequence::default();
    let mut out_position = 0;

    'match_loop: loop {
        // main match finder loop
        'inner_loop: loop
        {
            (ml, offset) = table.insert_and_get_longest_match_greedy(
                src,
                window_start,
                literals_before_match,
            );

            if window_start + WINDOW_SIZE + GLZ_MIN_MATCH + ml > src.len()
            {
                // close to input end
                break 'match_loop;
            }


            if offset != 0 && ml >= usize::from(GLZ_MIN_MATCH)
            {
                // found a match
                debug_assert!(window_start >= offset);
                let actual_offset = window_start - offset;

                sequence.ml = ml;
                sequence.ll = literals_before_match;
                sequence.ol = actual_offset;
                sequence.start = window_start - literals_before_match;

                break 'inner_loop;
            }

            let skip_literals = 1;

            literals_before_match += skip_literals;
            window_start += skip_literals;
        }

        literals_before_match = 0;


        if window_start + WINDOW_SIZE + UNCOMPRESSED > src.len()
        {
            // close to input end
            break 'match_loop;
        }
        table.advance(src, window_start, ml);
        window_start += ml;
        compress_sequence(src, dest, &mut out_position, &sequence);
    }
    sequence.ll = literals_before_match;
    sequence.ol = 0;
    sequence.start = window_start - literals_before_match;
    sequence.ml = GLZ_MIN_MATCH;
    compress_sequence(src,dest,&mut out_position,&sequence);
    return out_position;
}
