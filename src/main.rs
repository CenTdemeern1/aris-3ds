use std::time::{Duration, Instant};

use ctru::{prelude::*, services::gfx::{Flush, Swap, Screen}};

const FRAME_DURATION: Duration = Duration::new(0, 41666666); // 1 / 24

fn main() {
    let apt = Apt::new().unwrap();
    let mut hid = Hid::new().unwrap();
    let gfx = Gfx::new().unwrap();
    let _console = Console::new(gfx.top_screen.borrow_mut());

    println!("Decoding...");
    let decode_start = Instant::now();
    let _romfs = ctru::services::romfs::RomFS::new().unwrap();

    let mut aris_frames_decoded: Vec<Box<[u8]>> = Vec::with_capacity(17);
    for frame in 0..=16 {
        let mut aris_frame: Box<[u8]> = Box::new([0; 230400]);
        let _header = qoi::decode_to_buf(
            &mut aris_frame,
            std::fs::read(
                format!("romfs:/aris/aris{:02}.qoi", frame)
            ).unwrap()
        ).unwrap();
        aris_frames_decoded.push(aris_frame);
    }
    let decode_end = Instant::now();
    let decode_length = decode_end.duration_since(decode_start);
    println!("Decoding done! Took {} secs.", decode_length.as_secs_f64());

    let mut bottom_screen = gfx.bottom_screen.borrow_mut();

    let mut aris_frame_number = 0;

    println!("\n\nHello, World!");
    println!("\x1b[29;16HPress Start to exit");

    while apt.main_loop() {
        let frame_start = Instant::now();
        
        aris_frame_number += 1;
        aris_frame_number %= aris_frames_decoded.len();

        let frame_buffer = bottom_screen.raw_framebuffer();
        let aris = &aris_frames_decoded[aris_frame_number];

        unsafe {
            frame_buffer
                .ptr
                .copy_from(aris.as_ptr(), aris.len());
        }

        bottom_screen.flush_buffers();
        gfx.wait_for_vblank();
        bottom_screen.swap_buffers();

        hid.scan_input();
        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        let frame_end = Instant::now();
        let frame_duration = frame_end.duration_since(frame_start);
        std::thread::sleep(FRAME_DURATION - frame_duration);
    }
}
