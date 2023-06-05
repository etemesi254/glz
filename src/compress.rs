// //! The token format (control byte) is:
// //
// // OOLLLRRR
// //        └─Literal run length
// //     └────Match length
// // └──────Lower bits of match offset
//
// use varint_simd::encode;
//
// use crate::{LITERAL_BITS, MIN_MATCH, ML_BITS, OFFSET_BIT};
//
// pub struct Sequence
// {
//     literal_length: u32,
//     match_length:   usize,
//     num_seq_bytes:  u8,
//     offset:         u32
// }
//
// impl Sequence
// {
//     pub fn new(literal_length: u32, match_length: u32, offset: u32, num_seq_bytes: u8) -> Sequence
//     {
//         Sequence {
//             literal_length,
//             match_length,
//             offset,
//             num_seq_bytes
//         }
//     }
// }
//
// #[inline(always)]
// pub fn encode_sequence(seq: &Sequence, output: &mut [u8; 64]) -> u8
// {
//     // debug_assert!(seq.match_length >= MIN_MATCH);
//     let mut current_position = 1;
//     // encode lower bits of match offset
//     let lower_match_offset = (seq.offset & 0b11) as u8;
//
//     // ml token is given by (match length - 3(min_match_length));
//     let ml_token = (seq.match_length - MIN_MATCH).wrapping_sub(7);
//     let ll_token = seq.literal_length.wrapping_sub(7);
//
//     output[0] = lower_match_offset << OFFSET_BIT;
//
//     if seq.match_length >= (7 + MIN_MATCH)
//     {
//         // encode and add encode_mod
//         output[0] |= 7 << ML_BITS;
//         let extra = encode(ml_token);
//         // copy the whole var_int
//         output[1..17].copy_from_slice(&extra.0);
//         // increase the current position
//         current_position += extra.1;
//     }
//     else
//     {
//         // just encode the size
//         output[0] |= ((seq.match_length - MIN_MATCH) << ML_BITS) as u8;
//     }
//     if seq.literal_length < 7
//     {
//         output[0] |= (seq.literal_length << LITERAL_BITS) as u8;
//     }
//     else
//     {
//         output[0] |= 7 << LITERAL_BITS;
//
//         // encode using encode_mod
//         let extra = encode(ll_token);
//         // copy the whole var_int
//
//         // this helps remove the branch below
//         current_position &= 0b1111;
//         // copy the whole thing
//         output[usize::from(current_position)..usize::from(current_position + 16)]
//             .copy_from_slice(&extra.0);
//
//         // increase the current position
//         current_position += extra.1;
//     }
//
//     // TODO: Add literals here
//
//     if seq.offset >= 3
//     {
//         let extra = encode(seq.offset >> 2);
//         // copy the whole var_int
//
//         // this helps remove the branch below
//         current_position &= 0b11111;
//         // copy the whole thing
//         output[usize::from(current_position)..usize::from(current_position + 16)]
//             .copy_from_slice(&extra.0);
//
//         // increase the current position
//         current_position += extra.1;
//     }
//     current_position
// }
