/// Minimum match allowed by GLZ format
pub const GLZ_MIN_MATCH: usize = 3;
/// Hash finder window size
pub const WINDOW_SIZE: usize = 10;
/// Position of offset in token
pub const OFFSET_BIT: u8 = 6;
/// Position of literal in token
pub const LITERAL_BITS: u8 = 0;
/// Position of Match length in token
pub const ML_BITS: u8 = 3;
/// How many searches will be performed by the
/// match finder
pub const DEPTH_STRIDE: i32 = 20;
/// Extra bytes added to in and out
pub const SLOP_BYTES: usize = 1 << 16;
// Memory size
pub const MEM_SIZE: usize = 16 * (1 << 20);
/// Size of literal and match in token
pub const TOKEN: usize = 7;

pub const BLOCK_SIZE: usize = 1 << 18; //1 * (1 << 20);
