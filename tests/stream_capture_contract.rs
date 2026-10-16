//! FR: FR-006 / FR-007 — watch output must be observable before pipe EOF.

#[path = "common/stream_capture.rs"]
mod stream_capture;

use std::io::{pipe, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn publishes_complete_frames_while_writer_is_still_open() {
    let (reader, mut writer) = pipe().unwrap();
    let output = Arc::new(Mutex::new(String::new()));
    let captured = output.clone();
    let worker = thread::spawn(move || stream_capture::read_stream_into(reader, captured));
    writer.write_all(b"frame-one\nframe-two\n").unwrap();
    writer.flush().unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    let visible_before_eof = loop {
        if output.lock().unwrap().contains("frame-two\n") {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        thread::sleep(Duration::from_millis(10));
    };
    drop(writer);
    worker.join().unwrap();
    assert!(visible_before_eof, "watch frames must be published before the child exits");
    assert_eq!(&*output.lock().unwrap(), "frame-one\nframe-two\n");
}

#[test]
fn preserves_utf8_and_the_final_unterminated_frame() {
    let (reader, mut writer) = pipe().unwrap();
    let output = Arc::new(Mutex::new(String::new()));
    let captured = output.clone();
    let worker = thread::spawn(move || stream_capture::read_stream_into(reader, captured));
    writer.write_all("first π\nlast frame".as_bytes()).unwrap();
    drop(writer);
    worker.join().unwrap();
    assert_eq!(&*output.lock().unwrap(), "first π\nlast frame");
}
