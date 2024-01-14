use clap::{arg, command, value_parser};
use std::io::{self, Read};

const B: usize = 1600; // width of a Keccak-p permutation in bits
                       // this code only works for 1600: this is a feature
const BB: usize = B / 8; // B in bytes
const S: usize = 25; // bits per slice
const W: usize = B / S; // bits per lane
const L: usize = W.ilog2() as usize; // log2 of W
const R: usize = 256; // r in bits for shake128
                      // we assume that r is a multiple of 64
                      // MAKE SURE 0 < R < B
const RB: usize = R / 8; // r in bytes
const RW: usize = R / 64; // r in words
const C: usize = B - R; // c in bits
const CB: usize = C / 8; // c in bytes
const CW: usize = C / 64; // c in words
const DB: usize = 32; // D in bytes

type Lane = u64; // could use u32 for B = 800 for instance
type Plane = [Lane; 5]; // x z
type Sheet = [Lane; 5]; // y z
type State = [Sheet; 5]; // x y z
type SString = [Lane; S]; // (State string) string of B bits
type BString = [u8; BB]; // byte string of B bits

fn state_from_sstring(sstring: &SString) -> State {
    let mut state = [[0u64; 5]; 5];
    for x in 0..5 {
        for y in 0..5 {
            state[x][y] = sstring[5 * y + x];
        }
    }
    state
}

fn sstring_from_state(state: &State) -> SString {
    let mut sstring: SString = [0u64; S];
    for x in 0..5 {
        for y in 0..5 {
            sstring[5 * y + x] = state[x][y];
        }
    }
    sstring
}

/// xor all elements of array `a`
/// used in theta
fn xor_array(a: &[u64]) -> u64 {
    let mut res: u64 = 0;
    for e in a {
        res ^= e;
    }
    res
}

fn theta(a: &State) -> State {
    let mut c: Plane = [0u64; 5];
    let mut d: Plane = [0u64; 5];
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    for x in 0..5 {
        c[x] = xor_array(&a[x]);
    }
    for x in 0..5 {
        d[x] = c[(x + 4) % 5] ^ (c[(x + 1) % 5]).rotate_left(1); // rotate left to compensate for c[x+1 mod 5][(z-1) mod w] in FIPS
        for y in 0..5 {
            b[x][y] = a[x][y] ^ d[x];
        }
    }
    b
}

fn rho(a: &State) -> State {
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    b[0][0] = a[0][0];
    let (mut x, mut y) = (1, 0);
    for t in 0..24 {
        let offset = (((t + 1) * (t + 2) >> 1) % W) as u32;
        b[x][y] = (a[x][y]).rotate_left(offset); // rotate left for (z–(t+1)(t+2)/2) mod w in FIPS
        (x, y) = (y, (2 * x + 3 * y) % 5);
    }
    b
}

fn pi(a: &State) -> State {
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    for x in 0..5 {
        for y in 0..5 {
            b[x][y] = a[(x + 3 * y) % 5][x];
        }
    }
    b
}

fn chi(a: &State) -> State {
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    for x in 0..5 {
        for y in 0..5 {
            b[x][y] = a[x][y] ^ ((a[(x + 1) % 5][y] ^ u64::MAX) & a[(x + 2) % 5][y]);
        }
    }
    b
}

/// rc function defined in FIPS,
/// returns 0 or 1
fn rc(t: usize) -> Lane {
    if t % 255 == 0 {
        1
    } else {
        let mut r: Lane = 0b0000001;
        for _ in 1..=(t % 255) {
            r <<= 1; // r = 0 || r
            let r8: Lane = (r >> 8) & 1;
            if r8 == 1 {
                let mask: Lane = 0b01110001;
                r ^= mask;
            }
        }
        r & 1 // return r[0]
    }
}

fn iota(a: &State, ir: usize) -> State {
    let mut b: State = a.clone(); // a' in FIPS 202
    let mut rc_bits: Lane = 0;
    for j in 0..=L {
        if rc(j + 7 * ir) == 1 {
            rc_bits += 1 << ((1 << j) - 1);
        }
    }
    b[0][0] ^= rc_bits;
    b
}

fn round(a: &State, ir: usize) -> State {
    iota(&chi(&pi(&rho(&theta(&a)))), ir)
}

fn keccakp(s: &SString, nr: usize) -> SString {
    let mut a = state_from_sstring(&s);
    for ir in (12 + 2 * L - nr)..=(12 + 2 * L - 1) {
        a = round(&a, ir);
    }
    sstring_from_state(&a)
}

fn keccakf(s: &SString) -> SString {
    keccakp(&s, 12 + 2 * L)
}

fn sstate_from_buffer(buffer: &[u8; RB]) -> SString {
    let mut s: SString = [0u64; S];
    for i in 0..RW {
        // recall that RW = RB / 8
        for j in 0..8 {
            s[i] += (buffer[8 * i + j] as u64) << (8 * j);
        }
    }
    s
}

fn sponge_step_6(f: fn(&SString) -> SString, s: &mut SString, padded_pi: &SString) {
    let mut padded_pi_xor_s = [0u64; S];
    for i in 0..S {
        padded_pi_xor_s[i] = padded_pi[i] ^ s[i];
    }
    *s = f(&padded_pi_xor_s);
}

/// TODO: refactor
fn shake128_sponge(
    f: fn(&SString) -> SString,
    n_reader: fn(&mut [u8]) -> io::Result<usize>,
) -> [u8; DB] {
    let mut buffer = [0u8; RB];
    let mut s: SString = [0u64; S];
    loop {
        let read_bytes = n_reader(&mut buffer).unwrap();
        if read_bytes != RB {
            // add 1111 and pad with 10*1
            for i in read_bytes..RB {
                buffer[i] = 0;
            }
            buffer[read_bytes] += 0b11111000; // 1111 + first 1 of 10*1
            buffer[RB - 1] += 1;
        }
        let padded_pi = sstate_from_buffer(&buffer);
        sponge_step_6(f, &mut s, &padded_pi);
        if read_bytes != RB {
            break;
        }
    }
    // only works for RB = DB = 32
    let mut z = [0u8; DB];
    for i in 0..DB {
        z[i] = (s[i / 8] >> (8 * (i % 8))) as u8;
    }
    z
}

fn shake128(n_reader: fn(&mut [u8]) -> io::Result<usize>) -> [u8; DB] {
    shake128_sponge(keccakf, n_reader)
}

fn bytes_to_string(bytes: &[u8]) -> String {
    let mut s = String::new();
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn main() {
    let matches = command!()
        .arg(
            arg!([N] "Length of output in bytes")
                .required(true)
                .value_parser(value_parser!(u32)),
        )
        .get_matches();

    let out_len_bytes = matches.get_one::<u32>("N").unwrap();
    let res: [u8; 32] = shake128(|buffer| std::io::stdin().read(buffer));
    print!("{}", bytes_to_string(&res));
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// convert to State from String with format defined in
    /// https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-1600-IntermediateValues.txt
    fn state_from_xkcp_matrix(xkcp: &str) -> State {
        let lines: Vec<String> = xkcp.lines().map(str::to_string).collect();
        let mut words: Vec<Vec<String>> = Vec::new();
        for s in lines {
            words.push(s.split_whitespace().map(str::to_string).collect());
        }
        let mut s: State = [[0u64; 5]; 5];
        for x in 0..5 {
            for y in 0..5 {
                s[x][y] = u64::from_str_radix(&words[y][x], 16).unwrap();
            }
        }
        s
    }

    /// convert to SString from String with format defined in
    /// https://github.com/XKCP/XKCP/blob/master/tests/TestVectors/KeccakF-1600-IntermediateValues.txt
    fn bstring_from_xkcp_line(xkcp: &str) -> BString {
        let bytes: Vec<String> = xkcp.split_whitespace().map(str::to_string).collect();
        let mut b: BString = [0u8; BB];
        for i in 0..BB {
            b[i] = u8::from_str_radix(&bytes[i], 16).unwrap();
        }
        b
    }

    fn sstring_from_bstring(b: &BString) -> SString {
        let mut s: SString = [0u64; S];
        for i in 0..BB {
            s[i / 8] += (b[i] as u64) << (8 * (i % 8));
        }
        s
    }

    #[test]
    fn correct_keccakf() {
        let input_file = "tests/samples/keccakf_input";
        let expected_file = "tests/samples/keccakf_expected";
        let s = sstring_from_bstring(&bstring_from_xkcp_line(
            &fs::read_to_string(input_file).unwrap(),
        ));
        let output = super::keccakf(&s);
        let expected = sstring_from_bstring(&bstring_from_xkcp_line(
            &fs::read_to_string(expected_file).unwrap(),
        ));
        assert!(output == expected);
    }

    fn correct_permutation(
        permutation: fn(&State) -> State,
        input_file: &str,
        expected_file: &str,
    ) {
        let s = state_from_xkcp_matrix(&fs::read_to_string(input_file).unwrap());
        let output = permutation(&s);
        let expected = state_from_xkcp_matrix(&fs::read_to_string(expected_file).unwrap());
        assert!(output == expected);
    }

    #[test]
    fn correct_theta() {
        correct_permutation(
            super::theta,
            "tests/samples/theta_input_7",
            "tests/samples/theta_expected_7",
        )
    }

    #[test]
    fn correct_rho() {
        correct_permutation(
            super::rho,
            "tests/samples/theta_expected_7",
            "tests/samples/rho_expected_7",
        )
    }

    #[test]
    fn correct_pi() {
        correct_permutation(
            super::pi,
            "tests/samples/rho_expected_7",
            "tests/samples/pi_expected_7",
        )
    }

    #[test]
    fn correct_chi() {
        correct_permutation(
            super::chi,
            "tests/samples/pi_expected_7",
            "tests/samples/chi_expected_7",
        )
    }

    #[test]
    fn correct_iota() {
        correct_permutation(
            |s| super::iota(&s, 7),
            "tests/samples/chi_expected_7",
            "tests/samples/iota_expected_7",
        )
    }
}
