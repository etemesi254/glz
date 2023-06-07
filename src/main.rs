#![feature(slice_split_at_unchecked)]

use crate::compress::compress;
use crate::decompress::decompress;

mod compress;
mod constants;
mod decompress;
mod utils;

const HELP_MESSAGE: &str = "
USAGE
  glz [OPTIONS] <input_file> <output_file>

OPTIONS
    d Decompress input file into output file

ARGS:
    <input_directory> is the path to the directory with *.jpg files to be compressed (no nested subdirectories).
    <compressed_file> is the path to the archive file where the recompressed data of all input files is stored.
";

fn main()
{
    let mut pargs = pico_args::Arguments::from_env();

    if pargs.contains(["-h", "--help"])
    {
        print!("{}", HELP_MESSAGE);
        std::process::exit(0);
    }
    if let Ok(Some(sub)) = pargs.subcommand()
    {
        if sub == "d"
        {
            // decompression code
            let in_file: String = pargs.free_from_str().expect("Input file not given");
            let out_file: String = pargs.free_from_str().expect("Output  file not given");
            decompress(in_file, out_file);
        }
        else if sub == "c"
        {
            let in_file: String = pargs.free_from_str().expect("Input file not given");
            let out_file: String = pargs.free_from_str().expect("Output  file not given");
            compress(in_file, out_file);
        }
    }
    else
    {
        print!("No arguments passed quiting");
    }
}
