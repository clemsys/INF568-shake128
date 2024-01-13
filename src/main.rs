use clap::{arg, command, value_parser};

const B: usize = 1600; // width of a Keccak-p permutation in bits
                       // this code only works for 1600: this is a feature
const W: usize = B / 25; // bits per lane
const S: usize = B / W; // bits per slice

type Lane = u64; // u8, u16, u32, u64 respectively for B=200, 400, 800, 1600
type Plane = [Lane; 5]; // x z
type Sheet = [Lane; 5]; // y z
type State = [Sheet; 5]; // x y z
type SString = [Lane; S]; // (State string) string of B bits

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

fn xor_array(a: &[u64]) -> u64 {
    /// xor all elements of array `a`
    let mut res: u64 = 0;
    for e in a {
        res ^= e;
    }
    res
}

fn theta(a: State) -> State {
    let mut c: Plane = [0u64; 5];
    let mut d: Plane = [0u64; 5];
    let mut b: State = [[0u64; 5]; 5]; // a' in FIPS 202
    for x in 0..5 {
        c[x] = xor_array(&a[x]);
    }
    for x in 0..5 {
        d[x] = c[(x - 1) % 5] ^ (c[(x + 1) % 5]).rotate_left(1); // rotate left to compensate for c[x+1 mod 5][(z-1) mod w] in FIPS
        for y in 0..5 {
            b[x][y] = a[x][y] ^ d[x];
        }
    }
    b
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
    print!("9b171ccf7ff6b9478ce02a54a5a558dde55febc70e12f0ed402567639e404b74");
}
