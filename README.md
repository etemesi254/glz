## Format
 
- 4 bytes, block length , Little Endian, according to the original glz (provided by GDCC , should be smaller than 16 MB)
- Token
  - 2 bits, lower two bits of offset
  - 3 bits, literal token, if equals to 7(0b111), we will read more bytes to form the full literal
  - 3 bits, match token,  if equal to 7 (0b111), we will read more bytes to form the full match length
 
- If literal token is 7, we read more bytes, to form the full literal,  the scheme is `encode_mod`.
- After forming the literal token, we have the raw uncompressed literals of `n` bytes, (where `n` is the literal length)

- Offsets: use `encode_mod`, to form the full offset, remember to add token bytes and appropriate shift
- Match length, uses `encode_mod` ,to form full match length, add minimum match length allowed (3).

## Decode sequence
 - Decode token.
 - If literal in token is `7` add full length by decoding via `encode_mod`.
 - Copy raw literals from compressed buffer to uncompressed buffer, the length is given by the above decoded literal length
 - Decode offset via `encode_mod`, shift by one and add  the token offset.
 - Decode match length, if match token is `7`, decode via `encode_mod`, add `+3` for min match
 - Copy match.
 - Decode new token... 

## `encode_mod`

```Rust
/// Returns the decoded bytes and bytes consumed from encode-mod.
///
/// # Returns
/// - tuple1: Value
/// - tuple2: Number of bytes consumed
pub fn decode_encode_mod(input: &[u8]) -> (usize, usize)
{
    let mut curr_position = 0;
    let mut u_var = 0;
    let mut c_var;
    let mut value: usize = 0;
    loop
    {
        // SAFETY:  None :)
        let a = input[curr_position];
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
```
