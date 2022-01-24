use std::io::{Read, Write};

fn main() {
    let mut stdin = std::io::stdin();
    let mut buf = String::new();
    stdin.read_to_string(&mut buf).unwrap();

    for byte in buf.split(",") {
        let byte = byte.trim();
        if let Ok(byte) = byte.parse::<u8>() {
            std::io::stdout().write(&[byte]).unwrap();
        }
    }
}
