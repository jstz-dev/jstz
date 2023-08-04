use std::{
    fs::File,
    io::{stdin, Write},
    path::Path,
};

use jstz_serde::{into_inbox_array, InboxMessage};

fn main() {
    use std::fs::read_to_string;

    let mut args: Vec<_> = std::env::args().skip(1).collect();

    let target = if args.contains(&"--stdout".to_string()) {
        args.retain(|filename| filename != "--stdout");
        None
    } else {
        args.pop().filter(|arg| arg != "_")
    };

    let json = (if args.is_empty() {
        into_inbox_array(Some(InboxMessage::RunJs {
            code: stdin()
                .lines()
                .map(|line| line.expect("couldn't read stdin"))
                .collect(),
        }))
    } else {
        into_inbox_array(args.iter().map(|filename| InboxMessage::RunJs {
            code: read_to_string(filename).expect("can't open {filename}"),
        }))
    })
    .expect("can't decode json");

    match target {
        Some(filename) => {
            let path = Path::new(&filename);
            let mut file = File::create(path).expect("couldn't create file");
            file.write_all(json.as_bytes())
                .expect("couldn't write to file");
        }
        None => {
            println!("{json}");
        }
    }
}
