use ffav::easy::SimpleReader;
use std::convert::TryInto;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let early_exit = Arc::new(AtomicBool::new(false));
    let early_exit_cloned = Arc::clone(&early_exit);
    let early_exit_thread = std::thread::spawn(move || {
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();
        early_exit_cloned.store(true, Ordering::SeqCst);
    });

    let mut reader = SimpleReader::open("/tmp/envivio-352x288.264.mp4", None)?;
    for frame in reader.frames() {
        println!("frame={:#?}", frame);
        let bytes =
            unsafe { std::slice::from_raw_parts(frame.data, frame.size.try_into().unwrap()) };
        println!("bytes={:?}", &bytes[..16]);
    }

    println!("streams()={:#?}", reader.streams());
    for s in reader.streams() {
        println!("codecpar={:#?}", s.codecpar().unwrap());
    }

    early_exit_thread.join().unwrap();

    Ok(())
}
