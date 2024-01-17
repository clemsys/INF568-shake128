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

## Profiling

`shake128` can be profiled using `cargo flamegraph -- 32 < your_file`

## Optimizations

The `RC` array used in `iota` is computed at compile time and stored in the binary to improve performance. Same goes for `RHO_OFFSETS`, which is used in `rho`.

The file provided to `shake128` via the standard input is loaded chunk by chunk into the memory via a `BufReader`. This allows the user to run `./shake128` on very large files that would not fit into the memory. This also enables `shake128` to run with a very low peak memory usage, with a negligible impact on overall performance.

## Performance comparison with `openssl dgst -shake128`

`shake128` from this repo is about 75% slower than `openssl`. Here are some comparisons on commit `4cbb3b437db9a50e4272bf0f08b81ee0a6f4e63f`:

| file                                        | program                  | `time`                                          | peak memory usage |
| ------------------------------------------- | ------------------------ | ----------------------------------------------- | ----------------- |
| `ubuntu-22.04.3-desktop-amd64.iso` (`4,7G`) | `./shake128`             | `16,44s user 0,73s system 99% cpu 17,189 total` | 131.1 kB          |
|                                             | `openssl dgst -shake128` | `8,92s user 0,74s system 99% cpu 9,687 total`   | 786.4 kB          |
| `74.mp4` (`101M`)                           | `./shake128`             | `0,33s user 0,02s system 99% cpu 0,353 total`   | 131.1 kB          |
|                                             | `openssl dgst -shake128` | `0,19s user 0,02s system 99% cpu 0,203 total`   | 786.4 kB          |
