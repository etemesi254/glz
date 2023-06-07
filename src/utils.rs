#[inline(always)]
pub fn hash_chains_hash(
    bytes: &[u8], window_pos: usize, hash_length: usize, hash_log: usize
) -> usize
{
    // how many bytes to discard.
    let shift = (8 - hash_length) * 8;

    let bx = bytes
        .get(window_pos..window_pos + 8)
        .unwrap()
        .try_into()
        .unwrap();
    cache_table_inner_hash(bx, shift, hash_log) as usize
}

pub const fn cache_table_inner_hash(bytes: [u8; 8], shift_by: usize, shift_down_by: usize) -> u32
{
    // A stronger fmf_hash that has lesser fmf_hash collisions than
    // a simple multiplicative fmf_hash.
    let mut h = u64::from_le_bytes(bytes) << shift_by;
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    (h >> (64 - shift_down_by)) as u32
}

#[allow(unreachable_code)]
pub fn count(window: &[u8], match_window: &[u8]) -> usize
{
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse2"
    ))]
    {
        return count_sse(window, match_window);
    }
    count_fallback(window, match_window)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(any(target_feature = "sse2"))]
#[inline(always)]
pub fn count_sse(window: &[u8], match_window: &[u8]) -> usize
{
    /*
     * Note, all x64 processors support sse2, so as long as we compile for x86_64 we don't need
     * this to have a target_enable flag(might change tho).
     */
    // imports
    #[cfg(target_arch = "x86")]
    use std::arch::x86::{_mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8};
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::{_mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8};

    let mut offset = 0;

    let a_ptr = window.as_ptr();

    let b_ptr = match_window.as_ptr();

    let mut match_length = 0;

    // how many bytes can we go??
    // 128/8 -> 16
    let mut iterations = std::cmp::min(window.len(), match_window.len()) / 16;

    unsafe {
        // SAFETY: This is safe, because we checked we are running in a SSE capable processor
        while iterations > 0
        {
            let window_simd = _mm_loadu_si128(a_ptr.add(offset).cast());

            let match_simd = _mm_loadu_si128(b_ptr.add(offset).cast());

            let result = _mm_cmpeq_epi8(window_simd, match_simd);

            // mask uses lower 16 bits of int32, so let's convert
            // directly.
            let mask = _mm_movemask_epi8(result) as i16;

            match_length += mask.trailing_ones() as usize;

            if mask != -1
            {
                // -1, all data matched (0..16)
                // if there was a break, we already have
                // the longest match
                return match_length;
            }

            offset += 16;

            iterations -= 1;
        }
    }
    // sir we never thought we'd get here
    // long matches.
    match_length + count_fallback(&window[offset..], &match_window[offset..])
}

pub fn count_fallback(window: &[u8], match_window: &[u8]) -> usize
{
    /*
     * This is pretty neat and worth an explanation
     * a ^ b ==  0  if a==b
     *
     * If it's not zero the first non-zero bit will indicate that the byte at it's boundary is not the same
     *(e.g if bit 11 is 1 it means byte formed by bits [8..16] are not same). and if their not the same,
     * then our match stops there.
     *
     * Credits to Yann Collet lz4.c for this.
     */

    const SIZE: usize = usize::BITS as usize / 8;

    let mut match_length = 0;

    let window_chunks = window.chunks_exact(SIZE);

    let match_chunks = match_window.chunks_exact(SIZE);

    for (sm_window, sm_match) in window_chunks.zip(match_chunks)
    {
        let sm_w: usize = usize::from_ne_bytes(sm_window.try_into().unwrap());

        let sm_m: usize = usize::from_ne_bytes(sm_match.try_into().unwrap());

        let diff = sm_w ^ sm_m; // it's associative.

        if diff == 0
        {
            match_length += SIZE;
        }
        else
        {
            // naa they don't match fully
            match_length += (diff.trailing_zeros() >> 3) as usize;
            return match_length;
        }
    }

    // small chunks
    match_window[match_length..]
        .iter()
        .zip(&window[match_length..])
        .for_each(|(a, b)| {
            if a == b
            {
                match_length += 1;
            }
        });

    match_length
}

#[inline]
pub fn copy_literals(
    src: &[u8], dest: &mut [u8], src_offset: usize, dest_offset: usize, num_literals: usize
)
{
    const_copy::<16, true>(src, dest, src_offset, dest_offset);
    if num_literals > 16
    {
        let mut counter = 16;

        'num_literals: loop
        {
            const_copy::<16, true>(src, dest, src_offset + counter, dest_offset + counter);

            counter += 16;

            if counter >= num_literals
            {
                break 'num_literals;
            }
            // prevent optimizer from turning this into a memcpy
            // slows down speed due to overhead of function calls
            #[cfg(not(any(target_arch = "asmjs", target_arch = "wasm32")))]
            {
                use std::arch::asm;
                unsafe {
                    asm!("");
                }
            }
        }
    }
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
