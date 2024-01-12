use clap::{arg, command, value_parser};

fn main() {
    let matches = command!()
        .arg(
            arg!([N] "Length of output in bytes")
                .required(true)
                .value_parser(value_parser!(i32)),
        )
        .get_matches();

    let out_len_bytes = matches.get_one::<i32>("N").unwrap();
    println!("Value for name: {out_len_bytes}");
}
