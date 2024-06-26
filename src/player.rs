use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use std::{sync::mpsc, time::Duration};

static WIDTH: u32 = 1280;
static HEIGHT: u32 = 720;

pub fn render_video(rx: mpsc::Receiver<ffmpeg_next::frame::Video>) {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("bitwhip", WIDTH, HEIGHT)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, WIDTH, HEIGHT)
        .map_err(|e| e.to_string())
        .expect("No error");

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        texture
            .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                match rx.try_recv() {
                    Ok(frame) => {
                        buffer.clone_from_slice(frame.data(0));
                    }
                    Err(_err) => {
                    }
                }
            })
            .expect("texture copy");

        canvas.clear();
        canvas.copy(&texture, None, None).expect("No error");
        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
