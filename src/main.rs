#![feature(allocator_api)]

use std::{fs::File, io::{Read, Seek}, sync::{atomic::{AtomicBool, Ordering}, Arc, Barrier}, thread::{sleep, JoinHandle}, time::{Duration, Instant}};

use ctru::{
    linear::LinearAllocator, prelude::*, services::{
        gfx::{Flush, Screen, Swap},
        ndsp::{wave::{Status, Wave}, AudioFormat, AudioMix, InterpolationType, Ndsp, OutputMode}
    }
};

const FRAME_DURATION: Duration = Duration::new(0, 41666666); // 1 / 24
const AUDIO_CHUNK_SIZE: usize = 48000; // One fourth of a second's worth of audio samples
const AUDIO_CHECK_INTERVAL: Duration = Duration::new(0, 125000000); // 1 / 8
static RUN_AUDIO_THREAD: AtomicBool = AtomicBool::new(true);

fn fill_audio_buffer_from_file(buffer: &mut [u8], file: &mut File) {
    if file.read(buffer).unwrap() == 0 {
        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        file.read(buffer).unwrap();
    }
}

fn main() {
    let apt = Apt::new().unwrap();
    let mut hid = Hid::new().unwrap();
    let gfx = Gfx::new().unwrap();
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let _romfs = ctru::services::romfs::RomFS::new().unwrap();

    println!("Decoding...");
    let decode_start = Instant::now();

    let barrier = Arc::new(Barrier::new(2));
    let thread_barrier = barrier.clone();

    let audio_thread_join_handle: JoinHandle<()> = std::thread::spawn(move || {
        if let Ok(mut ndsp) = Ndsp::new() {
            let mut usagi_flap_file = std::fs::File::open("romfs:/usagi-flap.pcm").unwrap();
            ndsp.set_output_mode(OutputMode::Stereo);
            let mut channel = ndsp.channel(0).unwrap();
            channel.set_format(AudioFormat::PCM16Stereo);
            channel.set_interpolation(InterpolationType::Linear);
            channel.set_sample_rate(48000.);
            let mix = AudioMix::default();
            channel.set_mix(&mix);
            let mut audio_pcm: [[u8; AUDIO_CHUNK_SIZE]; 2] = [[0u8; AUDIO_CHUNK_SIZE]; 2];
            fill_audio_buffer_from_file(&mut audio_pcm[0], &mut usagi_flap_file);
            fill_audio_buffer_from_file(&mut audio_pcm[1], &mut usagi_flap_file);
            let mut audio_pcm: [Wave; 2] = [
                Wave::new(
                    Box::new_in(audio_pcm[0], LinearAllocator),
                    AudioFormat::PCM16Stereo,
                    false
                ),
                Wave::new(
                    Box::new_in(audio_pcm[1], LinearAllocator),
                    AudioFormat::PCM16Stereo,
                    false
                )
            ];
            thread_barrier.wait();
            channel.queue_wave(&mut audio_pcm[0]).unwrap();
            channel.queue_wave(&mut audio_pcm[1]).unwrap();
            let mut buffer_to_use = 0usize;
            while RUN_AUDIO_THREAD.load(Ordering::Relaxed) {
                // IMPORTANT: The all important yield_now!
                // Commenting out both the yield_now and the sleep will make the application run slow and deadlock the system when pressing the HOME button.
                // Having either one uncommented actually makes the thread yield correctly resolving both the aforementioned problems.
                // sleep(AUDIO_CHECK_INTERVAL);
                std::thread::yield_now();
                let current = &mut audio_pcm[buffer_to_use];
                if let Status::Done = current.status() {
                    // Get audio data from file and put it in the buffer!
                    let mutable_buffer = current.get_buffer_mut().unwrap();
                    fill_audio_buffer_from_file(mutable_buffer, &mut usagi_flap_file);
                    // Queue the buffer!
                    channel.queue_wave(current).unwrap();
                    // Change which buffer to fill next!
                    buffer_to_use += 1;
                    // Make sure it doesn't increase beyond the amount of buffers
                    // This means we can triple buffer, or quadruple buffer if you really want to
                    buffer_to_use %= audio_pcm.len();
                }
            }
        } else {
            println!("\x1b[33mWarning: NDSP firmware not found.\nContinuing without sound.\x1b[0m");
            thread_barrier.wait();
        }
    });

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

    barrier.wait();

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
        sleep(FRAME_DURATION.saturating_sub(frame_duration));
    }

    RUN_AUDIO_THREAD.store(false, Ordering::Relaxed);
    audio_thread_join_handle.join().unwrap(); // Make sure the thread can exit because of the co-operative threading model
}
