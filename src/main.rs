use clap::{arg, command, value_parser};

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
        d[x] = c[(x - 1) % 5] ^ (c[(x + 1) % 5]).rotate_left(1); // rotate left to compensate for c[x+1 mod 5][(z-1) mod w] in FIPS
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
            b[x][y] = a[x + 3 * y][x];
        }
    }
    b
}

fn xi(a: &State) -> State {
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
            rc_bits += 1 << (1 << j - 1);
        }
    }
    b[0][0] ^= rc_bits;
    b
}

fn round(a: &State, ir: usize) -> State {
    iota(&xi(&pi(&rho(&theta(&a)))), ir)
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
