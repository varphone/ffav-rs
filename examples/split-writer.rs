use ffav::easy::{AudioDesc, OpenOptions, VideoDesc};
use std::convert::TryInto;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let early_exit = Arc::new(AtomicBool::new(false));
    let early_exit_cloned = Arc::clone(&early_exit);
    let early_exit_thread = std::thread::spawn(move || {
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();
        early_exit_cloned.store(true, Ordering::SeqCst);
    });

    let a_desc = AudioDesc::new();
    let v_desc = VideoDesc::with_h264(352, 288, 4000, 1000000);
    let example_bytes = include_bytes!("envivio-352x288.264.framed");
    let mut ts_writer = OpenOptions::new()
        .media(a_desc)
        .media(v_desc)
        .format("mpegts")
        .format_options("mpegts_copyts=1")
        .max_files(10)
        .max_size_bytes(1024 * 1024)
        .max_size_time(10_000_000_000)
        .start_index(100)
        .open("/tmp/")?;

    let mut pts = 0;

    for _n in 0..100 {
        if early_exit.load(Ordering::SeqCst) {
            break;
        }

        let mut offset: usize = 0;

        while offset + 4 < example_bytes.len() {
            if early_exit.load(Ordering::SeqCst) {
                break;
            }
            let size_bytes = &example_bytes[offset..offset + 4];
            let frame_size = i32::from_be_bytes(size_bytes.try_into().unwrap()) as usize;
            offset += 4;
            let frame_bytes = &example_bytes[offset..offset + frame_size];
            offset += frame_size;
            ts_writer.write_bytes(frame_bytes, pts, 40000, false, 0)?;
            pts += 40000;
        }
    }

    early_exit_thread.join().unwrap();

    Ok(())
}
