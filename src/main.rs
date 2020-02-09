#![feature(try_trait)]

use ffmpeg4::{format::Pixel, frame};
use std::path::Path;
use num_complex::Complex64;

mod generator;
mod output;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 90;
const FRAMES: u32 = 300;
const PLANE_WIDTH: f64 = 3f64;
const IMAGE_SCALE: f64 = PLANE_WIDTH / WIDTH as f64;
const PLANE_START_X: f64 = -PLANE_WIDTH / 2f64;
const PLANE_START_Y: f64 = -(HEIGHT as f64 * IMAGE_SCALE) / 2f64;

fn main() {
    let media_file = Path::new("fractal.webm");
    let mut media_out = output::MediaOutput::new(&media_file, WIDTH, HEIGHT, (1, 30))
        .expect("Unable to open a media output");
    media_out.start().expect("Unable to start the media file");

    let mut frame = frame::Video::new(Pixel::RGBA, WIDTH, HEIGHT);
    for frame_num in 0..FRAMES {
        frame.set_pts(Some(frame_num as i64));

        let c_offset = frame_num as f64 / FRAMES as f64 * 2f64 - 1f64;
        let c = Complex64::new(c_offset, c_offset);
        let generator = generator::ValueGenerator::new(IMAGE_SCALE, IMAGE_SCALE, PLANE_START_X, PLANE_START_Y, false, 500, c);

        let frame_data = frame.data_mut(0);
        let fractal_image = generator::generate_fractal(&generator, WIDTH, HEIGHT, num_cpus::get() + 2, |progress| {
            println!("Fractal Generation Progress:");
            print!(" ");
            for f in progress {
                print!(" {:.2}%", f * 100f32);
            }
            println!();
        }).expect("Error generating fractal");

        frame_data.copy_from_slice(&fractal_image);

        println!("Writing frame: {}", frame_num);
        media_out
            .write_frame(&frame)
            .expect("Unable to write a frame to the media file");
    }
    println!("Finishing...");
    media_out
        .finish()
        .expect("Unable to finish writing the media file");
    println!("Done.");
}
