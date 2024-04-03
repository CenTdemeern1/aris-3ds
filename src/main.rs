use std::time::{Duration, Instant};

use ctru::{prelude::*, services::gfx::{Flush, Swap, Screen}};

const FRAME_DURATION: Duration = Duration::new(0, 41666666); // 1 / 24

fn main() {
    let apt = Apt::new().unwrap();
    let mut hid = Hid::new().unwrap();
    let gfx = Gfx::new().unwrap();
    let _console = Console::new(gfx.top_screen.borrow_mut());

    println!("Hello, World!");
    println!("\x1b[29;16HPress Start to exit");

    let _romfs = ctru::services::romfs::RomFS::new().unwrap();

    let mut aris_frames_encoded: Vec<Vec<u8>> = Vec::with_capacity(17);
    for frame in 0..=16 {
        aris_frames_encoded.push(
            std::fs::read(
                format!("romfs:/aris/aris{:02}.qoi", frame)
            ).unwrap()
        );
    }
    // let mut aris_frames_encoded: Vec<Vec<u8>> = vec![
    //     include_bytes!("../romfs/aris/aris00.qoi").into()
    // ];

    let mut bottom_screen = gfx.bottom_screen.borrow_mut();

    let mut aris: Box<[u8]> = Box::new([0; 230400]);

    let mut aris_frame_number = 0;

    while apt.main_loop() {
        let frame_start = Instant::now();

        // gfx.wait_for_vblank();
        bottom_screen.swap_buffers();
        
        aris_frame_number += 1;
        aris_frame_number %= aris_frames_encoded.len();
        
        println!("Starting decode of frame {}", aris_frame_number);
        let decode_start = Instant::now();
        let _header = qoi::decode_to_buf(&mut aris, &aris_frames_encoded[aris_frame_number]).unwrap();
        let decode_end = Instant::now();
        let decode_length = decode_end.duration_since(decode_start);
        println!("Decoding of frame {} done. Took {} secs.", aris_frame_number, decode_length.as_secs_f64());

        let frame_buffer = bottom_screen.raw_framebuffer();

        unsafe {
            frame_buffer
                .ptr
                .copy_from(aris.as_ptr(), aris.len());
        }

        bottom_screen.flush_buffers();

        hid.scan_input();
        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        let frame_end = Instant::now();
        let frame_duration = frame_end.duration_since(frame_start);
        // std::thread::sleep(FRAME_DURATION - frame_duration);
    }
}
