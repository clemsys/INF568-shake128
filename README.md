# INF568 Assignment 1 - shake128

Author: [Clément CHAPOT](mailto:clement.chapot@polytechnique.edu) <br>
Description: implementation of shake128 (see: [FIPS 202](https://csrc.nist.gov/pubs/fips/202/final)) as part of INF568 course at École polytechnique

## Building

Build the project using `make`.

This calls `cargo build --release` and copies `target/release/shake128` in the project root.

## Running

Run using `./shake128 <N>`.

`shake128` reads from the standard input stream, and writes the resulting hash value to standard output. For more usage information, run `./shake128 --help`.
