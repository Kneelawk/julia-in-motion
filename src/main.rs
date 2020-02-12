#![feature(try_trait)]

use crate::generator::ValueGenerator;
use ffmpeg4::{format::Pixel, frame};
use num_complex::{Complex, Complex64};
use std::{fs::create_dir_all, num::ParseIntError, path::Path};

mod generator;
mod output;
mod util;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FRAMES: u32 = 300;
const PLANE_WIDTH: f64 = 3f64;
const IMAGE_SCALE: f64 = PLANE_WIDTH / WIDTH as f64;
const PLANE_START_X: f64 = -PLANE_WIDTH / 2f64;
const PLANE_START_Y: f64 = -(HEIGHT as f64 * IMAGE_SCALE) / 2f64;

fn main() {
    // load up option parser
    let options_yaml = clap::load_yaml!("options.yml");
    let matches = clap::App::from_yaml(options_yaml)
        .version(clap::crate_version!())
        .get_matches();

    // parse all the options
    let image_width = matches
        .value_of("image_width")
        .unwrap()
        .parse::<u32>()
        .expect("Unable to parse --image-width <WIDTH> argument as an integer");
    let image_height = matches
        .value_of("image_height")
        .unwrap()
        .parse::<u32>()
        .expect("Unable to parse --image-height <HEIGHT> argument as an integer");
    let frames = matches
        .value_of("frames")
        .unwrap()
        .parse::<u32>()
        .expect("Unable to parse --frames <FRAME_COUNT> argument as an integer");
    let plane_width = matches
        .value_of("plane_width")
        .unwrap()
        .parse::<f64>()
        .expect("Unable to parse --plane-width <WIDTH> argument as a number");

    // parse the output file and create its parent directories if needed
    let output = Path::new(matches.value_of("output").unwrap());
    if let Some(parent) = output.parent() {
        if !parent.exists() {
            create_dir_all(parent);
        }
    }

    // parse the path string as an SVG path
    let path_str = matches.value_of("path").unwrap();
    let svg_builder = lyon_path::Path::builder().with_svg();
    let path = lyon_svg::path_utils::build_path(svg_builder, path_str)
        .expect("Unable to parse --path <SVG_PATH> using SVG path syntax");

    // get the optional arguments
    let fractal_progress_interval = matches
        .value_of("fractal_progress_interval")
        .unwrap()
        .parse::<u32>()
        .expect(
            "Unable to parse --fractal-progress-interval <MILLISECONDS> argument as an integer",
        );
    let video_progress_interval = matches
        .value_of("video_progress_interval")
        .unwrap()
        .parse::<u32>()
        .expect("Unable to parse --video-progress-interval <MILLISECONDS> argument as an integer");
    let time_base = util::parse_rational(matches
        .value_of("time_base")
        .unwrap())
        .expect("Unable to parse --time-base <FRACTION> argument as a fraction");

    // get the flags
    let mandelbrot = matches.is_present("mandelbrot");

    let mut media_out = output::MediaOutput::new(&output, image_width, image_height, time_base);

    //    let media_file = Path::new("fractal.webm");
    //    let mut media_out = output::MediaOutput::new(&media_file, WIDTH,
    // HEIGHT, (1, 30))        .expect("Unable to open a media output");
    //    media_out.start().expect("Unable to start the media file");
    //
    //    let mut frame = frame::Video::new(Pixel::RGBA, WIDTH, HEIGHT);
    //    for frame_num in 0..FRAMES {
    //        frame.set_pts(Some(frame_num as i64));
    //
    //        let c_offset = frame_num as f64 / FRAMES as f64 * 2f64 - 1f64;
    //        let c = Complex64::new(c_offset, c_offset);
    //        let generator = generator::ValueGenerator::new(
    //            IMAGE_SCALE,
    //            IMAGE_SCALE,
    //            PLANE_START_X,
    //            PLANE_START_Y,
    //            false,
    //            100,
    //            c,
    //        );
    //
    //        let frame_data = frame.data_mut(0);
    //        let fractal_image = generator::generate_fractal(
    //            &generator,
    //            WIDTH,
    //            HEIGHT,
    //            num_cpus::get() + 2,
    //            |progress| {
    //                println!("Fractal Generation Progress:");
    //                print!(" ");
    //                for f in progress {
    //                    print!(" {:.2}%", f * 100f32);
    //                }
    //                println!();
    //            },
    //        )
    //        .expect("Error generating fractal");
    //
    //        frame_data.copy_from_slice(&fractal_image);
    //
    //        println!("Writing frame: {}", frame_num);
    //        media_out
    //            .write_frame(&frame)
    //            .expect("Unable to write a frame to the media file");
    //    }
    //    println!("Finishing...");
    //    media_out
    //        .finish()
    //        .expect("Unable to finish writing the media file");
    //    println!("Done.");
}

/// Draws a crosshair at the specified location in the complex plane onto a u8
/// buffer representing an RGBA image.
fn draw_crosshair(
    image: &mut [u8],
    image_width: usize,
    image_height: usize,
    crosshair_coordinates: Complex<f64>,
    generator: &ValueGenerator,
) {
    let (pixel_x, pixel_y) = generator.get_pixel_coordinates(crosshair_coordinates);

    for x in 0..image_width {
        let index = (pixel_y as usize * image_width + x) * 4;
        image[index] = 0xFFu8;
        image[index + 1] = 0xFFu8;
        image[index + 2] = 0xFFu8;
        image[index + 3] = 0xFFu8;
    }

    for y in 0..image_height {
        let index = (y * image_width + pixel_x as usize) * 4;
        image[index] = 0xFFu8;
        image[index + 1] = 0xFFu8;
        image[index + 2] = 0xFFu8;
        image[index + 3] = 0xFFu8;
    }
}
