use clap::Parser;
use std::{fs::File, io::Write};

#[derive(Parser, Debug)]
struct Args {
    path: String,
}

fn main() {
    File::create(Args::parse().path)
        .expect("Could not open path for writing")
        .write(
            jstz_node::docs()
                .to_pretty_json()
                .expect("Could not generate json")
                .as_bytes(),
        )
        .expect("Could not write to file");
}
