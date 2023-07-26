use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use crate::constants::{GLZ_MIN_MATCH, LITERAL_BITS, MEM_SIZE, ML_BITS, OFFSET_BIT, SLOP_BYTES};
use crate::utils::{const_copy, fixed_copy_within};

const TOKEN_LITERAL: usize = 32;
const TOKEN_MATCH_LENGTH: usize = 32;

#[inline(always)]
pub fn decode_encode_mod(input: &[u8]) -> (usize, usize)
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

pub fn decode_sequences(
    input: &[u8], output_size: usize, output: &mut [u8]
) -> Result<usize, &'static str>
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
        let mut match_length = (usize::from(token >> ML_BITS) & 0b111) + GLZ_MIN_MATCH;
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
                let mut tmp_src_offset = input_offset + TOKEN_LITERAL;
                let mut tmp_dst_offset = output_offset + TOKEN_LITERAL;

                let mut ll_copy = literal_length;

                'literals: loop
                {
                    const_copy::<TOKEN_LITERAL, false>(
                        input,
                        output,
                        tmp_src_offset,
                        tmp_dst_offset
                    );
                    tmp_src_offset += TOKEN_LITERAL;
                    tmp_dst_offset += TOKEN_LITERAL;

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
        if input_offset > output_size
        {
            return Err("Corrupt file");
        }

        // extract match offset
        let (ol, consumed_offset) = decode_encode_mod(&input[input_offset..]);

        offset |= (ol << 2) as usize;

        let match_start = output_offset - offset;

        if offset > output_offset
        {
            return Err("Corrupt file");
        }

        //dbg!(literal_length, match_length, match_start, output_offset);

        // increment the input to point to match
        input_offset += usize::from(consumed_offset);
        // extract the match length
        if match_length == (7 + GLZ_MIN_MATCH)
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
            let mut dst_position = output_offset + offset;

            // loop copying offset bytes in place
            // notice this loop does fixed copies but increments in offset bytes :)
            // that is intentional.
            loop
            {
                if dst_position > output_offset + match_length
                {
                    break;
                }

                fixed_copy_within::<TOKEN_MATCH_LENGTH>(output, src_position, dst_position);

                src_position += offset;
                dst_position += offset;
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
        }

        output_offset += match_length;
    }

    return Ok(output_offset);
}

pub fn decompress(input_file: String, output_file: String)
{
    let p = Path::new(&input_file);

    let p_len = p.metadata().unwrap().len() as usize;
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

    let mut curr_len = 0;
    let mut file_contents = [0; 4];
    let mut end_position = 0;

    while curr_len < p_len
    {
        fd.read_exact(&mut file_contents).unwrap();
        let size = u32::from_le_bytes(file_contents[0..4].try_into().unwrap()) as usize;
        fd.read_exact(&mut max_in[0..size]).unwrap();

        match decode_sequences(&max_in, size as usize, &mut max_out)
        {
            Ok(f_length) =>
            {
                curr_len += size as usize + 4 /*size bytes*/;
                end_position += f_length;
                out_fd.write_all(&max_out[..f_length]).unwrap();
                out_fd.flush().unwrap();
            }
            Err(str) =>
            {
                println!("{}", str);
                return;
            }
        }
    }

    let end = Instant::now();

    println!("{curr_len}->{end_position} in {:?}", end - start)
}
