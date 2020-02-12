#![feature(try_trait)]

use ffmpeg4::{format, frame};
use lyon_algorithms::{walk, walk::RegularPattern};
use lyon_path::iterator::PathIterator;
use num_complex::Complex;
use std::{
    fs::create_dir_all,
    path::Path,
    time::{Duration, Instant},
};

mod generator;
mod output;
mod path_length;
mod util;

#[derive(Copy, Clone)]
struct FractalValues {
    pub image_width: u32,
    pub image_height: u32,
    pub plane_width: f64,
    pub plane_height: f64,
    pub image_scale: f64,
    pub plane_start_x: f64,
    pub plane_start_y: f64,
    pub iterations: u32,
}

impl FractalValues {
    pub fn new(
        image_width: u32,
        image_height: u32,
        plane_width: f64,
        iterations: u32,
    ) -> FractalValues {
        let image_scale = plane_width / image_width as f64;
        let plane_height = image_height as f64 * image_scale;

        FractalValues {
            image_width,
            image_height,
            plane_width,
            plane_height,
            image_scale,
            plane_start_x: -plane_width / 2f64,
            plane_start_y: -plane_height / 2f64,
            iterations,
        }
    }

    pub fn get_pixel_coordinates(
        &self,
        plane_coordinates: Complex<f64>,
    ) -> (Option<u32>, Option<u32>) {
        (
            if plane_coordinates.re > self.plane_start_x
                && plane_coordinates.re < self.plane_start_x + self.plane_width
            {
                Some(((plane_coordinates.re - self.plane_start_x) / self.image_scale) as u32)
            } else {
                None
            },
            if plane_coordinates.im > self.plane_start_y
                && plane_coordinates.im < self.plane_start_y + self.plane_height
            {
                Some(((plane_coordinates.im - self.plane_start_y) / self.image_scale) as u32)
            } else {
                None
            },
        )
    }
}

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
            create_dir_all(parent).expect("Unable to create video output parent directory");
        }
    }

    // parse the path string as an SVG path
    let path_str = matches.value_of("path").unwrap();
    let svg_builder = lyon_path::Path::builder().with_svg();
    let path = lyon_svg::path_utils::build_path(svg_builder, path_str)
        .expect("Unable to parse --path <SVG_PATH> using SVG path syntax");

    // get the optional arguments
    let iterations = matches
        .value_of("iterations")
        .unwrap()
        .parse::<u32>()
        .expect("Unable to parse --iterations <ITERATIONS> argument as an integer");
    let fractal_progress_interval = Duration::from_millis(
        matches
            .value_of("fractal_progress_interval")
            .unwrap()
            .parse::<u64>()
            .expect(
                "Unable to parse --fractal-progress-interval <MILLISECONDS> argument as an integer",
            ),
    );
    let video_progress_interval = Duration::from_millis(
        matches
            .value_of("video_progress_interval")
            .unwrap()
            .parse::<u64>()
            .expect(
                "Unable to parse --video-progress-interval <MILLISECONDS> argument as an integer",
            ),
    );
    let time_base = util::parse_rational(matches.value_of("time_base").unwrap())
        .expect("Unable to parse --time-base <FRACTION> argument as a fraction");

    // get the path tolerance
    let path_tolerance = matches
        .value_of("path_tolerance")
        .unwrap()
        .parse::<f32>()
        .expect("Unable to parse --path-tolerance <TOLERANCE> argument as a number");

    // get the flags
    let mandelbrot = matches.is_present("mandelbrot");

    // open the media output
    let mut media_out = output::MediaOutput::new(&output, image_width, image_height, time_base)
        .expect("Unable to open the media output");
    media_out.start().expect("Unable to start the media file");

    let fractal_arguments = FractalValues::new(image_width, image_height, plane_width, iterations);

    // walk along the path to determine its length
    let path_length = path_length::approximate_path_length(path.as_slice(), path_tolerance);

    // get the length of each step
    let step_length = path_length / frames as f32;

    let video_progress_callback = |frame_num| {
        println!("Generated {} frames out of {}", frame_num, frames);
    };

    let fractal_progress_callback = |progress| {
        println!("Fractal Generation Progress:");
        print!(" ");
        for f in progress {
            print!(" {:.2}%", f * 100f32);
        }
        println!();
    };

    if mandelbrot {
        render_mandelbrot(
            fractal_arguments,
            &mut media_out,
            path,
            path_tolerance,
            step_length,
            video_progress_interval,
            fractal_progress_interval,
            &video_progress_callback,
            &fractal_progress_callback,
        );
    } else {
        render_julia(
            fractal_arguments,
            &mut media_out,
            path,
            path_tolerance,
            step_length,
            video_progress_interval,
            fractal_progress_interval,
            &video_progress_callback,
            &fractal_progress_callback,
        );
    }

    media_out
        .finish()
        .expect("Error finishing writing media file");
}

/// Renders the video as a Mandelbrot set with crosshairs tracing a path along
/// it.
fn render_mandelbrot<V: Fn(u32), F: Fn(Vec<f32>)>(
    vals: FractalValues,
    media_out: &mut output::MediaOutput,
    path: lyon_path::Path,
    path_tolerance: f32,
    step_length: f32,
    video_progress_interval: Duration,
    fractal_progress_interval: Duration,
    video_progress_callback: &V,
    fractal_progress_callback: &F,
) {
    let generator = generator::ValueGenerator::new(
        vals.image_scale,
        vals.image_scale,
        vals.plane_start_x,
        vals.plane_start_y,
        true,
        vals.iterations,
        Complex::<f64>::new(0f64, 0f64),
    );

    let mandelbrot_image = generator::generate_fractal(
        &generator,
        vals.image_width,
        vals.image_height,
        num_cpus::get() + 2,
        fractal_progress_callback,
        fractal_progress_interval,
    )
    .expect("Error generating Mandelbrot set");

    let mut frame = frame::Video::new(format::Pixel::RGBA, vals.image_width, vals.image_height);
    let mut frame_num = 0;
    let mut previous_progress = Instant::now();

    let mut pattern = RegularPattern {
        callback: &mut |position: lyon_algorithms::math::Point, _, _| {
            frame.set_pts(Some(frame_num as i64));
            let mut current_image = mandelbrot_image.clone();

            draw_crosshair(
                &mut current_image,
                vals,
                Complex::<f64>::new(position.x as f64, position.y as f64),
            );

            frame.data_mut(0).copy_from_slice(&current_image);

            media_out
                .write_frame(&frame)
                .expect("Unable to write frame");

            // call the progress callback every now and then
            let now = Instant::now();
            if now.saturating_duration_since(previous_progress) > video_progress_interval {
                video_progress_callback(frame_num);
                previous_progress = now;
            }

            frame_num += 1;

            true
        },
        interval: step_length,
    };

    walk::walk_along_path(path.iter().flattened(path_tolerance), 0f32, &mut pattern);
}

/// Renders the video as a Julia set following the specified path along the
/// Mandelbrot set.
fn render_julia<V: Fn(u32), F: Fn(Vec<f32>)>(
    vals: FractalValues,
    media_out: &mut output::MediaOutput,
    path: lyon_path::Path,
    path_tolerance: f32,
    step_length: f32,
    video_progress_interval: Duration,
    fractal_progress_interval: Duration,
    video_progress_callback: &V,
    fractal_progress_callback: &F,
) {
    let mut frame = frame::Video::new(format::Pixel::RGBA, vals.image_width, vals.image_height);
    let mut frame_num = 0;
    let mut previous_progress = Instant::now();

    let mut pattern = RegularPattern {
        callback: &mut |position: lyon_algorithms::math::Point, _, _| {
            frame.set_pts(Some(frame_num as i64));

            let generator = generator::ValueGenerator::new(
                vals.image_scale,
                vals.image_scale,
                vals.plane_start_x,
                vals.plane_start_y,
                false,
                vals.iterations,
                Complex::<f64>::new(position.x as f64, position.y as f64),
            );

            let julia_image = generator::generate_fractal(
                &generator,
                vals.image_width,
                vals.image_height,
                num_cpus::get() + 2,
                fractal_progress_callback,
                fractal_progress_interval,
            )
            .expect("Error generating Julia set");

            frame.data_mut(0).copy_from_slice(&julia_image);

            media_out
                .write_frame(&frame)
                .expect("Unable to write frame");

            // call the progress callback every now and then
            let now = Instant::now();
            if now.saturating_duration_since(previous_progress) > video_progress_interval {
                video_progress_callback(frame_num);
                previous_progress = now;
            }

            frame_num += 1;

            true
        },
        interval: step_length,
    };

    walk::walk_along_path(path.iter().flattened(path_tolerance), 0f32, &mut pattern);
}

/// Draws a crosshair at the specified location in the complex plane onto a u8
/// buffer representing an RGBA image.
fn draw_crosshair(image: &mut [u8], vals: FractalValues, crosshair_coordinates: Complex<f64>) {
    let (pixel_x, pixel_y) = vals.get_pixel_coordinates(crosshair_coordinates);

    if let Some(pixel_y) = pixel_y {
        for x in 0..vals.image_width as usize {
            let index = (pixel_y as usize * vals.image_width as usize + x) * 4;
            image[index] = 0xFFu8;
            image[index + 1] = 0xFFu8;
            image[index + 2] = 0xFFu8;
            image[index + 3] = 0xFFu8;
        }
    }

    if let Some(pixel_x) = pixel_x {
        for y in 0..vals.image_height as usize {
            let index = (y * vals.image_width as usize + pixel_x as usize) * 4;
            image[index] = 0xFFu8;
            image[index + 1] = 0xFFu8;
            image[index + 2] = 0xFFu8;
            image[index + 3] = 0xFFu8;
        }
    }
}
