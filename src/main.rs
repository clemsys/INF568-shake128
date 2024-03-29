use clap::{arg, command, value_parser};
use std::io::{Read, StdinLock};

const B: usize = 1600; // width of a Keccak-p permutation in bits
                       // this code only works for 1600: this is a feature
const S: usize = 25; // bits per slice
const W: usize = B / S; // bits per lane
const L: usize = W.ilog2() as usize; // log2 of W
const SHAKE128_CB: usize = 256 >> 3; // c in bytes for shake128

type Lane = u64; // could use u32 for B = 800 for instance
type Plane = [Lane; 5]; // x z
type Sheet = [Lane; 5]; // y z
type State = [Sheet; 5]; // x y z
type SString = [Lane; S]; // (State string) string of B bits

fn state_from_sstring(sstring: &[u64]) -> State {
    assert!(sstring.len() == S);
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

/// compute offset used in rho
const fn offset(t: usize) -> u32 {
    ((((t + 1) * (t + 2)) >> 1) % W) as u32
}

/// generate the offsets used in rho at compilation time
const fn generate_rho_offsets() -> [[u32; 5]; 5] {
    let mut t_indexed_offsets = [0u32; 25];
    let mut t_indexed_xy = [0usize; 25]; // xy[i] = (x << 3) + y
    let mut i = 0;
    while i < 24 {
        // use while because for is not supported in const fn
        t_indexed_offsets[i] = offset(i);
        if i == 0 {
            t_indexed_xy[0] = 8;
        } else {
            let y = t_indexed_xy[i - 1] % 8;
            let x = t_indexed_xy[i - 1] >> 3;
            t_indexed_xy[i] = (y << 3) + ((2 * x + 3 * y) % 5);
        }
        i += 1;
    }
    let mut offsets = [[0u32; 5]; 5];
    let mut i = 0;
    while i < 24 {
        let y = t_indexed_xy[i] % 8;
        let x = t_indexed_xy[i] >> 3;
        offsets[x][y] = t_indexed_offsets[i];
        i += 1;
    }
    offsets
}

/// offsets used in rho computed at compilation time (to improve performance)
/// we could have has well copied the values from FIPS 202 manually
const RHO_OFFSETS: [[u32; 5]; 5] = generate_rho_offsets();

fn rho(a: &State) -> State {
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    for x in 0..5 {
        for y in 0..5 {
            if x != 0 || y != 0 {
                b[x][y] = (a[x][y]).rotate_left(RHO_OFFSETS[x][y]); // rotate left for (z–(t+1)(t+2)/2) mod w in FIPS
            } else {
                b[0][0] = a[0][0];
            }
        }
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
const fn rc_generator(t: usize) -> bool {
    assert!(t < 255);
    if t == 0 {
        true
    } else {
        let mask: u8 = 0b01110001;
        let mut r: u8 = 0b0000001;
        let mut i = 1;
        while i <= t {
            // use while because for is not supported in const fn
            let r8 = r & 0b10000000;
            r <<= 1; // r = 0 || r
            if r8 != 0 {
                r ^= mask;
            }
            i += 1;
        }
        (r & 1) == 1 // return r[0]
    }
}

/// generate RC at compilation time for performance reasons
const fn generate_rc() -> [bool; 255] {
    let mut t = 0;
    let mut rc = [true; 255];
    while t < 255 {
        // use while because for is not supported in const fn
        rc[t] = rc_generator(t);
        t += 1;
    }
    rc
}

/// RC is computed at compilation time for performance reasons
const RC: [bool; 255] = generate_rc();

fn rc(t: usize) -> bool {
    RC[t % 255]
}

fn iota(a: &State, ir: usize) -> State {
    let mut b: State = *a; // a' in FIPS 202
    let mut rc_bits: Lane = 0;
    for j in 0..=L {
        if rc(j + 7 * ir) {
            rc_bits += 1 << ((1 << j) - 1);
        }
    }
    b[0][0] ^= rc_bits;
    b
}

fn round(a: &State, ir: usize) -> State {
    iota(&chi(&pi(&rho(&theta(a)))), ir)
}

fn keccakp(s: &[u64], nr: usize) -> SString {
    assert!(s.len() == S);
    let mut a = state_from_sstring(s);
    for ir in (12 + 2 * L - nr)..=(12 + 2 * L - 1) {
        a = round(&a, ir);
    }
    sstring_from_state(&a)
}

fn keccakf(s: &[u64]) -> SString {
    assert!(s.len() == S);
    keccakp(s, 12 + 2 * L)
}

/// groups bytes 8 by 8 to form an array of u64
fn words_from_bytes(b: &[u8]) -> Vec<u64> {
    assert!(b.len() % 8 == 0);
    let mut w = vec![0u64; b.len() >> 3]; // divide by 8
    for i in 0..(b.len() >> 3) {
        w[i] = u64::from_le_bytes(b[(i << 3)..((i + 1) << 3)].try_into().unwrap());
    }
    w
}

/// transform each u64 in input array in eight u8
fn bytes_from_words(w: &[u64]) -> Vec<u8> {
    let mut b = vec![0u8; w.len() << 3];
    for i in 0..(b.len()) {
        b[i] = (w[i >> 3] >> ((i % 8) << 3) & 0xff) as u8;
    }
    b
}

/// computes the output of the sponge on the content read by reader thanks to padding_read
/// with permutation f <br>
/// rb is r in bytes, db is d in bytes
fn sponge(
    f: fn(&[u64]) -> SString,
    rb: usize,
    padding_read: fn(&mut Vec<u8>, &mut StdinLock) -> bool,
    reader: &mut StdinLock,
    db: usize,
) -> Vec<u8> {
    let rw = rb >> 3; // r in 64-bits words
    let cw = (B >> 6) - rw; // c in words
    let mut s = [0u64; S];
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        buffer.clear();
        let remaining_data = padding_read(&mut buffer, reader);
        let mut p_i = words_from_bytes(&buffer);
        p_i.append(&mut vec![0u64; cw]);
        assert!(p_i.len() == S);
        let mut padded_s_xor_p_i = [0u64; S];
        for j in 0..S {
            padded_s_xor_p_i[j] = s[j] ^ p_i[j];
        }
        s = f(&padded_s_xor_p_i);
        if !remaining_data {
            break;
        }
    }
    let mut z: Vec<u8> = Vec::new();
    while z.len() < db {
        z.append(&mut bytes_from_words(&s[0..rw]));
        s = f(&s);
    }
    z.truncate(db);
    z
}

/// computes keccak[c] on the content read by reader thanks to padding_read <br>
/// cb is c in bytes, db is d in bytes
fn keccak(
    cb: usize,
    padding_read: fn(&mut Vec<u8>, &mut StdinLock) -> bool,
    reader: &mut StdinLock,
    db: usize,
) -> Vec<u8> {
    sponge(keccakf, (B >> 3) - cb, padding_read, reader, db)
}

/// computes the shake128 on the content read by reader thanks to padding_read <br>
/// db is d in bytes
fn shake128(
    padding_read: fn(&mut Vec<u8>, &mut StdinLock) -> bool,
    reader: &mut StdinLock,
    db: usize,
) -> Vec<u8> {
    keccak(SHAKE128_CB, padding_read, reader, db)
}

/// takes RB bytes from the reader and puts them in the buffer
/// and adds padding bytes at the end for shake128 <br>
/// returns whether stdin still holds unread bytes
fn padding_read(buffer: &mut Vec<u8>, reader: &mut StdinLock) -> bool {
    let capacity = B / 8 - SHAKE128_CB;
    let read_bytes = reader.take(capacity as u64).read_to_end(buffer).unwrap();
    if read_bytes == capacity {
        true
    } else {
        buffer.push(0b00011111);
        for _ in (read_bytes + 1)..capacity {
            buffer.push(0);
        }
        buffer[capacity - 1] += 0b10000000;
        false
    }
}

/// formats the (large) integer stored in a bytes array as a string of hexadecimal digits
fn string_from_bytes(bytes: &[u8]) -> String {
    let mut s = String::new();
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn main() {
    // deal with command line arguments
    let matches = command!()
        .arg(
            arg!([N] "Length of output in bytes")
                .required(true)
                .value_parser(value_parser!(usize)),
        )
        .get_matches();
    // convert CLI argument to usize (output length in bytes)
    let db = *matches.get_one::<usize>("N").unwrap();

    // create (buffered) reader to read bytes from stdin
    let mut reader = std::io::stdin().lock();

    // compute the shake128
    let res = shake128(padding_read, &mut reader, db);
    print!("{}", string_from_bytes(&res));
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const BB: usize = B >> 3; // b in bytes
    type BString = [u8; BB]; // byte string of B bits

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

    #[test]
    fn correct_keccakf() {
        let input_file = "tests/samples/keccakf_input";
        let expected_file = "tests/samples/keccakf_expected";
        let s = words_from_bytes(&bstring_from_xkcp_line(
            &fs::read_to_string(input_file).unwrap(),
        ));
        let output = super::keccakf(&s);
        let expected = words_from_bytes(&bstring_from_xkcp_line(
            &fs::read_to_string(expected_file).unwrap(),
        ));
        assert_eq!(output, &expected[0..]);
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
