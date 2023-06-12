use crate::compress::hash_chains::hash_chains::HashChains;
use crate::compress::EncodeSequence;
use crate::constants::{
    GLZ_MIN_MATCH, LITERAL_BITS, ML_BITS, OFFSET_BIT, SKIP_TRIGGER, TOKEN, WINDOW_SIZE
};
use crate::utils::copy_literals;

pub mod hash_chains;

#[inline(always)]
fn write_token(seq: &EncodeSequence) -> u8
{
    debug_assert!(seq.ml >= GLZ_MIN_MATCH);
    let ml_length = seq.ml - GLZ_MIN_MATCH;

    let ml_token = if ml_length >= 7 { 7 } else { ml_length as u8 };
    let ll_token = if seq.ll >= 7 { 7 } else { seq.ll as u8 };
    let ol_token = (seq.ol & 0b11) as u8;

    let mut out: u8 = 0;

    out |= ol_token << OFFSET_BIT;
    out |= ml_token << ML_BITS;
    out |= ll_token << LITERAL_BITS;

    out
}

fn compress_encode_mod(mut value: usize, dest: &mut [u8], dest_position: &mut usize)
{
    let mut left: i32;

    if value > 0x7f
    {
        loop
        {
            dest[*dest_position] = ((value & 255) | 0x80) as u8;
            *dest_position += 1;
            // debugging purposes
            // left = (<usize as TryInto<i32>>::try_into(value).unwrap()) - 0x80;
            // value = (<i32 as TryInto<usize>>::try_into(left).unwrap()) >> 7;
            left = (value as i32) - 0x80;
            value = left as usize >> 7;

            if value <= 0x7f
            {
                break;
            }
        }
    }
    dest[*dest_position] = value as u8;
    *dest_position += 1;
}

fn compress_sequence<const IS_END: bool>(
    src: &[u8], dest: &mut [u8], dest_position: &mut usize, seq: &EncodeSequence
)
{
    let start = *dest_position;
    let token_byte = write_token(seq);
    let mut extra = *seq;

    extra.ll = extra.ll.wrapping_sub(7);
    extra.ml = extra.ml.wrapping_sub(7 + GLZ_MIN_MATCH);
    extra.ol = extra.ol >> 2;

    dest[*dest_position] = token_byte;
    *dest_position += 1;

    if seq.ll >= TOKEN
    {
        compress_encode_mod(extra.ll, dest, dest_position);
    }

    // copy literals
    copy_literals(src, dest, seq.start, *dest_position, seq.ll);
    *dest_position += seq.ll;

    if IS_END
    {
        return;
    }

    // encode offset
    compress_encode_mod(extra.ol, dest, dest_position);

    if seq.ml >= TOKEN + GLZ_MIN_MATCH
    {
        // encode long ml
        compress_encode_mod(extra.ml, dest, dest_position);
    }
    let end = *dest_position;
    let token_b = end - start - seq.ll;
    assert_ne!(seq.start + seq.ll, seq.ol);
    assert!(token_b <= seq.ml, "{token_b}, {end} {start} {}", seq.ml);
}

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
