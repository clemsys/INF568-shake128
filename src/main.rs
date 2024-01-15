use clap::{arg, command, value_parser};
use std::io::Read;

const B: usize = 1600; // width of a Keccak-p permutation in bits
                       // this code only works for 1600: this is a feature
const S: usize = 25; // bits per slice
const W: usize = B / S; // bits per lane
const L: usize = W.ilog2() as usize; // log2 of W

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

fn rho(a: &State) -> State {
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    b[0][0] = a[0][0];
    let (mut x, mut y) = (1, 0);
    for t in 0..24 {
        let offset = ((((t + 1) * (t + 2)) >> 1) % W) as u32;
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
    let mut b: State = *a; // a' in FIPS 202
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

fn words_from_bytes(b: &[u8]) -> Vec<u64> {
    assert!(b.len() % 8 == 0);
    let mut w = vec![0u64; b.len() >> 3]; // divide by 8
    for i in 0..(b.len()) {
        w[i >> 3] += (b[i] as u64) << ((i % 8) << 3);
    }
    w
}

fn bytes_from_words(w: &[u64]) -> Vec<u8> {
    let mut b = vec![0u8; w.len() << 3];
    for i in 0..(b.len()) {
        b[i] = (w[i >> 3] >> ((i % 8) << 3) & 0xff) as u8;
    }
    b
}

/// padding is done by shake128
/// db in bytes
fn sponge(f: fn(&[u64]) -> SString, rb: usize, p: &[u8], db: usize) -> Vec<u8> {
    let rw = rb >> 3; // r in 64-bits words
    let cw = (B >> 6) - rw; // c in words
    assert!(p.len() % rb == 0);
    let mut s = [0u64; S];
    for i in 0..(p.len() / rb) {
        let mut p_i = words_from_bytes(&p[(rb * i)..(rb * (i + 1))]);
        p_i.append(&mut vec![0u64; cw]);
        assert!(p_i.len() == S);
        let mut padded_s_xor_p_i = [0u64; S];
        for j in 0..S {
            padded_s_xor_p_i[j] = s[j] ^ p_i[j];
        }
        s = f(&padded_s_xor_p_i);
    }
    let mut z: Vec<u8> = Vec::new();
    while z.len() < db {
        z.append(&mut bytes_from_words(&s[0..rw]));
        s = f(&s);
    }
    z.truncate(db);
    z
}

/// padding is done by shake128
/// cb, db in bytes
fn keccak(cb: usize, p: &[u8], db: usize) -> Vec<u8> {
    sponge(keccakf, (B >> 3) - cb, p, db)
}

/// cb, db in bytes
fn shake128(message: &mut Vec<u8>, db: usize) -> Vec<u8> {
    let cb = 128 >> 3;
    let rb = (B >> 3) - cb;
    // pad message
    message.push(0b00011111);
    while message.len() % rb != 0 {
        message.push(0);
    }
    let mlen = message.len();
    message[mlen - 1] += 0b10000000;
    keccak(cb, message, db)
}

fn string_from_bytes(bytes: &[u8]) -> String {
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
                .value_parser(value_parser!(usize)),
        )
        .get_matches();

    let db = *matches.get_one::<usize>("N").unwrap(); // output length in bytes
    let mut message: Vec<u8> = Vec::new();
    std::io::stdin().read_to_end(&mut message).unwrap();
    let res = shake128(&mut message, db);
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
