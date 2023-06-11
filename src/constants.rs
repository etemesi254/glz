/// Minimum match allowed by GLZ format
pub const GLZ_MIN_MATCH: usize = 3;
/// Hash finder window size
pub const WINDOW_SIZE: usize = 5;
/// Position of offset in token
pub const OFFSET_BIT: u8 = 6;
/// Position of literal in token
pub const LITERAL_BITS: u8 = 0;
/// Position of Match length in token
pub const ML_BITS: u8 = 3;
/// Compression level used by match finder to calculate depth
/// increasing this will lead to increase in compression but slower compression
pub const COMPRESSION_LEVEL: usize = 60;
/// Number of bytes we may consider uncompressed
pub const UNCOMPRESSED: usize = 32;
/// How many searches will be performed by the
/// match finder
pub const DEPTH_STRIDE: usize = 6;
/// Minimal match considered in the match finder
///
/// Smaller allows more matches, larger less matches with more
pub const HASH_CHAINS_MINIMAL_MATCH: usize = 4;
/// log2 number of buckets available during encoding
pub const HASH_CHAINS_BUCKET_LOG: usize = 16;
/// Extra bytes added to in and out
pub const SLOP_BYTES: usize = 1 << 16;
// Memory size
pub const MEM_SIZE: usize = 16 * (1 << 20);
/// Size of literal and match in token
pub const TOKEN: usize = 7;

pub const SKIP_TRIGGER: usize = 13;
