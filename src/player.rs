use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use std::sync::mpsc;

pub fn render_video(rx: mpsc::Receiver<ffmpeg_next::frame::Video>) {
    match rx.recv() {
        Ok(first_frame) => {
            let sdl_context = sdl2::init().unwrap();
            let video_subsystem = sdl_context.video().unwrap();
            let window = video_subsystem
                .window("bitwhip", first_frame.width(), first_frame.height())
                .position_centered()
                .build()
                .unwrap();

            let mut canvas = window.into_canvas().build().unwrap();
            let mut event_pump = sdl_context.event_pump().unwrap();
            let texture_creator = canvas.texture_creator();
            let mut texture = texture_creator
                .create_texture_streaming(PixelFormatEnum::IYUV, first_frame.width(), first_frame.height())
                .map_err(|e| e.to_string())
                .expect("No error");

            let buffer_size: i32;
            unsafe {
                buffer_size = ffmpeg_sys_next::av_image_get_buffer_size(
                    first_frame.format().into(),
                    first_frame.width() as i32,
                    first_frame.height() as i32,
                    32,
                );
            };


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
                                unsafe {
                                    let frame_ptr = *frame.as_ptr();
                                    ffmpeg_sys_next::av_image_copy_to_buffer(
                                        buffer.as_mut_ptr(),
                                        buffer_size,
                                        frame_ptr.data.as_ptr() as *mut _,
                                        frame_ptr.linesize.as_ptr() as *mut _,
                                        frame.format().into(),
                                        frame_ptr.width,
                                        frame_ptr.height,
                                        32,
                                    );
                                }
                            }
                            Err(_err) => {}
                        }
                    })
                .expect("texture copy");

                canvas.clear();
                canvas.copy(&texture, None, None).expect("No error");
                canvas.present();
            }
        }
        Err(_err) => {}
    }
}
