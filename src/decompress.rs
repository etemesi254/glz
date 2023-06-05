use std::cell::Cell;

use crate::{LITERAL_BITS, MIN_MATCH, ML_BITS, OFFSET_BIT};

const TOKEN_LITERAL: usize = 32;
const TOKEN_MATCH_LENGTH: usize = 32;

#[inline(always)]
fn decode_encode_mod(input: &[u8]) -> (usize, usize)
{
    let mut curr_position = 0;
    let mut u_var = 0;
    let mut c_var;
    let mut value: usize = 0;
    loop
    {
        // SAFETY:  None :)
        let a = *unsafe { input.get_unchecked(curr_position) };
        c_var = usize::from(a) << (u_var & 0x1f);
        value = value + c_var;
        curr_position += 1;

        if a <= 0x7f
        {
            break;
        }
        u_var += 7;
    }
    return (value, curr_position);
}

pub fn decode_sequences(input: &[u8], output_size: usize, output: &mut [u8]) -> usize
{
    let mut input_offset = 0;
    let mut output_offset = 0;

    loop
    {
        if input_offset == output_size
        {
            break;
        }
        // read the next token
        let token = unsafe { *input.get_unchecked(input_offset) };

        // extract bytes from token
        let mut offset = usize::from(token >> OFFSET_BIT) & 0b011;
        let mut match_length = (usize::from(token >> ML_BITS) & 0b111) + MIN_MATCH;
        let mut literal_length = usize::from(token >> LITERAL_BITS) & 0b111;

        // increment the input by one to signify we read a token
        input_offset += 1;
        // read literals
        if literal_length == 7
        {
            // too long of a literal, decode using EncodeMod
            let (ll, b) = decode_encode_mod(&input[input_offset..]);
            input_offset += usize::from(b);
            literal_length += ll as usize;

            // unchecked copy of a literal, helps in copying literals greater than 7
            // but less than 16
            const_copy::<TOKEN_LITERAL, false>(input, output, input_offset, output_offset);

            if literal_length >= TOKEN_LITERAL
            {
                // copy literals longer than 16
                let mut src_offset_copy = input_offset + TOKEN_LITERAL;

                let mut tmp_dest_copy = TOKEN_LITERAL + output_offset;
                let mut ll_copy = literal_length;

                'literals: loop
                {
                    const_copy::<TOKEN_LITERAL, false>(
                        input,
                        output,
                        src_offset_copy,
                        tmp_dest_copy
                    );
                    src_offset_copy += TOKEN_LITERAL;
                    tmp_dest_copy += TOKEN_LITERAL;

                    if ll_copy < TOKEN_LITERAL + TOKEN_LITERAL
                    {
                        // 64 because
                        // a. we copied TOKEN_LITERAL length already(out of the loop)
                        // b. we copied TOKEN_LITERAL bytes above
                        break 'literals;
                    }
                    ll_copy = ll_copy.wrapping_sub(TOKEN_LITERAL);
                }
            }
        }
        else
        {
            const_copy::<8, false>(input, output, input_offset, output_offset);
        }
        // increment the input to point past the literals
        input_offset += literal_length;
        output_offset += literal_length;
        // check if we are done
        if input_offset == output_size
        {
            break;
        }

        // extract match offset
        let (ol, consumed_offset) = decode_encode_mod(&input[input_offset..]);

        offset |= (ol << 2) as usize;

        let match_start = output_offset - offset;

        // increment the input to point to match
        input_offset += usize::from(consumed_offset);
        // extract the match length
        if match_length == (7 + MIN_MATCH)
        {
            // too long of a match, decode using EncodeMod
            let (ml, b) = decode_encode_mod(&input[input_offset..]);
            input_offset += usize::from(b);
            match_length += ml as usize;
        }

        // copy the match
        let (dest_src, dest_ptr) = unsafe { output.split_at_mut_unchecked(output_offset) };

        // Copy the match length.
        // (Copies matches of up to 32 bytes)
        const_copy::<TOKEN_MATCH_LENGTH, false>(dest_src, dest_ptr, match_start, 0);

        if offset <= TOKEN_MATCH_LENGTH
        {
            // the unconditional copy above copied some bytes
            // don't let it go into waste
            // Increment the position we are in by the number of correct bytes
            // currently copied
            let mut src_position = match_start + offset;
            let mut dest_position = output_offset + offset;

            // loop copying offset bytes in place
            // notice this loop does fixed copies but increments in offset bytes :)
            // that is intentional.
            loop
            {
                fixed_copy_within::<TOKEN_MATCH_LENGTH>(output, src_position, dest_position);

                src_position += offset;
                dest_position += offset;

                if dest_position > output_offset + match_length
                {
                    break;
                }
            }
            // overlapping match
        }
        else if match_length >= TOKEN_MATCH_LENGTH
        {
            // // we had copied TOKEN_MATCH_LENGTH bytes initially, do not recopy them
            let mut tmp_src_pos = TOKEN_MATCH_LENGTH + match_start;
            let mut tmp_dst_pos = TOKEN_MATCH_LENGTH;

            let mut ml_copy = match_length;
            // copy in batches of 32
            'match_lengths: loop
            {
                const_copy::<TOKEN_MATCH_LENGTH, false>(
                    dest_src,
                    dest_ptr,
                    tmp_src_pos,
                    tmp_dst_pos
                );

                tmp_src_pos += TOKEN_MATCH_LENGTH;
                tmp_dst_pos += TOKEN_MATCH_LENGTH;

                if ml_copy < TOKEN_MATCH_LENGTH + TOKEN_MATCH_LENGTH
                {
                    break 'match_lengths;
                }

                ml_copy = ml_copy.wrapping_sub(TOKEN_MATCH_LENGTH);
            }
            //output.copy_within(match_start..match_start + match_length, output_offset);
        }

        output_offset += match_length;
    }

    return output_offset;
}

pub fn const_copy<const SIZE: usize, const SAFE: bool>(
    src: &[u8], dest: &mut [u8], src_offset: usize, dest_offset: usize
)
{
    // ensure we don't go out of bounds(only if SAFE is true)
    if SAFE
    {
        assert!(
            src_offset + SIZE - 1 < src.len(),
            "End position {} our of range for slice of length {}",
            src_offset + SIZE,
            src.len()
        );
        assert!(
            dest_offset + SIZE < dest.len(),
            "End position {} our of range for slice of length {}",
            dest_offset + SIZE,
            dest.len()
        );
    }
    unsafe {
        dest.as_mut_ptr()
            .add(dest_offset)
            .copy_from(src.as_ptr().add(src_offset), SIZE);
        // do not generate calls to memcpy optimizer
        // I'm doing some exclusive shit
        // (If it's a loop, the optimizer may lift this to be a memcpy)
        #[cfg(not(any(target_arch = "asmjs", target_arch = "wasm32")))]
        {
            use std::arch::asm;
            asm!("");
        }
    }
}

pub fn fixed_copy_within<const SIZE: usize>(dest: &mut [u8], src_offset: usize, dest_offset: usize)
{
    // for debug builds ensure we don't go out of bounds
    debug_assert!(
        dest_offset + SIZE <= dest.len(),
        "[dst]: End position {} out of range for slice of length {}",
        dest_offset + SIZE,
        dest.len()
    );

    dest.copy_within(src_offset..src_offset + SIZE, dest_offset);
}
