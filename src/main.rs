use clap::{arg, command, value_parser};

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
