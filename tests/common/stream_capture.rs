use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex};

pub fn read_stream_into(reader: impl Read, output: Arc<Mutex<String>>) {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                output.lock().expect("capture lock").push_str(&line);
                line.clear();
            }
        }
    }
}
