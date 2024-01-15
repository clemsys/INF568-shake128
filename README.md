# INF568 Assignment 1 - shake128

Author: [Clément CHAPOT](mailto:clement.chapot@polytechnique.edu) <br>
Description: implementation of shake128 (see: [FIPS 202](https://csrc.nist.gov/pubs/fips/202/final)) as part of INF568 course at École polytechnique

## Building

Build the project using `make`.

This calls `cargo build --release` and copies `target/release/shake128` in the project root.

## Running

Run using `./shake128 <N>`.

`shake128` reads from the standard input stream, and writes the resulting hash value to standard output. For more usage information, run `./shake128 --help`.

## Testing

Run `cargo test` to test if `shake128` produces the right output.

In particular, `correct_short_text` and `correct_short_binary` check whether my implementation and the implementation from the [`sha3` crate](https://crates.io/crates/sha3) (which is only a dev dependency) give the same result.

`cargo test` also runs unit tests (for permutations, …) based on the data from the [XKCP github repo](https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/)
