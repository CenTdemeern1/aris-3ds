#![feature(allocator_api)]

use std::{io::Read, sync::atomic::{AtomicBool, Ordering}, time::{Duration, Instant}};

use ctru::{
    linear::LinearAllocator, prelude::*, services::{
        gfx::{Flush, Screen, Swap},
        ndsp::{wave::Wave, AudioFormat, InterpolationType, Ndsp, OutputMode}
    }
};

const FRAME_DURATION: Duration = Duration::new(0, 41666666); // 1 / 24
static RUN_AUDIO_THREAD: AtomicBool = AtomicBool::new(true);

fn main() {
    let apt = Apt::new().unwrap();
    let mut hid = Hid::new().unwrap();
    let gfx = Gfx::new().unwrap();
    let ndsp = Ndsp::new();
    let _console = Console::new(gfx.top_screen.borrow_mut());

    let audio_thread_join_handle = if let Ok(ndsp) = ndsp {
        Some(std::thread::spawn(|| {
            while RUN_AUDIO_THREAD.load(Ordering::Relaxed) {
                // Buffer size should be 24431616, but that's too big I'm going to have to do some buffer manipulation here maybe
                let mut usagi_flap_pcm: Box<[u8], LinearAllocator> = Box::new_in([0u8; 1048576], LinearAllocator);
                std::fs::File::open("romfs:/usagi-flap.pcm").unwrap().read_exact(&mut usagi_flap_pcm).unwrap();
                let mut usagi_flap = Wave::new(
                    usagi_flap_pcm,
                    AudioFormat::PCM16Stereo,
                    true
                );
                ndsp.set_output_mode(OutputMode::Stereo);
                let mut channel = ndsp.channel(0).unwrap();
                channel.set_format(AudioFormat::PCM16Stereo);
                channel.set_interpolation(InterpolationType::None);
                channel.set_sample_rate(48000.);
                channel.queue_wave(&mut usagi_flap).unwrap();
            }
        }))
    } else {
        println!("\x1b[33mWarning: NDSP firmware not found.\nContinuing without sound.\x1b[0m");
        None
    };

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

    println!("\n\nHello, World! I present to you: Aris");
    println!("\n(Animation by BlueSechi, song is Usagi Flap)");
    println!("Homebrew by CenTdemeern1, written in Rust");
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

    if Some(audio_thread_join_handle) = audio_thread_join_handle {
        RUN_AUDIO_THREAD.store(false, Ordering::Relaxed);
        audio_thread_join_handle.join(); // Make sure the thread can exit because of the co-operative threading model
    }
}
