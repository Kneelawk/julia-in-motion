#![feature(try_trait)]

use ffmpeg4::{format, frame};
use num_complex::Complex;
use rusttype::{Font, Scale};
use std::time::{Duration, Instant};

mod args;
mod generator;
mod output;
mod path_util;
mod raster;
mod util;

const FONT_DATA: &[u8] = include_bytes!("OxygenMono-Regular.ttf");

fn main() {
    let cmd_args = args::CmdArgs::load().expect("Error parsing commandline args");

    let font = Font::from_bytes(FONT_DATA).expect("Error loading font");

    let mut app = Application::new(cmd_args, font).expect("Error creating the application");

    app.run().expect("Error running the application");
}

struct Application<'a> {
    view: generator::view::View,
    iterations: u32,
    smoothing: generator::args::Smoothing,
    mandelbrot: bool,
    font: Font<'a>,
    media_out: output::MediaOutput,
    frames: u32,
    path: lyon_path::Path,
    path_tolerance: f32,
    step_length: f32,
    video_progress_interval: Duration,
    fractal_progress_interval: Duration,
}

impl Application<'_> {
    pub fn new(args: args::CmdArgs, font: Font) -> Result<Application, ApplicationCreationError> {
        // open the media output
        let media_out = output::MediaOutput::new(
            &args.output,
            args.image_width,
            args.image_height,
            args.time_base,
        )?;

        // walk along the path to determine its length
        let path_length =
            path_util::approximate_path_length(args.path.as_slice(), args.path_tolerance);

        // get the length of each step
        let step_length = path_length / args.frames as f32;

        Ok(Application {
            view: generator::view::View::new_uniform(
                args.image_width,
                args.image_height,
                args.plane_width,
            ),
            iterations: args.iterations,
            smoothing: args.smoothing,
            mandelbrot: args.mandelbrot,
            font,
            media_out,
            frames: args.frames,
            path: args.path,
            path_tolerance: args.path_tolerance,
            step_length,
            video_progress_interval: args.video_progress_interval,
            fractal_progress_interval: args.fractal_progress_interval,
        })
    }

    pub fn run(&mut self) -> Result<(), ApplicationRunError> {
        self.media_out.start()?;

        if self.mandelbrot {
            self.render_mandelbrot()?;
        } else {
            self.render_julia()?;
        }

        self.media_out.finish()?;

        Ok(())
    }

    /// Renders the video as a Mandelbrot set with crosshairs tracing a path
    /// along it.
    fn render_mandelbrot(&mut self) -> Result<(), ApplicationRunError> {
        let generator = generator::ValueGenerator::new(
            self.view,
            true,
            self.iterations,
            self.smoothing,
            Complex::<f64>::new(0f64, 0f64),
        );

        let mandelbrot_image = generator::generate_fractal(
            &generator,
            num_cpus::get() + 2,
            |progress| self.fractal_progress_callback(progress),
            self.fractal_progress_interval,
        )?;

        let mut frame = frame::Video::new(
            format::Pixel::RGBA,
            self.view.image_width,
            self.view.image_height,
        );
        let mut frame_num = 0;
        let mut previous_progress = Instant::now();

        let points =
            path_util::path_points(self.path.as_slice(), self.path_tolerance, self.step_length);

        for position in points {
            frame.set_pts(Some(frame_num as i64));
            let mut current_image = mandelbrot_image.clone();

            let complex = Complex::<f64>::new(position.x as f64, position.y as f64);
            let (pixel_x, pixel_y) = self.view.get_pixel_coordinates(complex);

            raster::draw_constrained_crosshair(
                &mut current_image,
                self.view.image_width,
                self.view.image_height,
                (pixel_x, pixel_y),
            );

            let complex_str = format!("{:.5} + {:.5}i", complex.re, complex.im);
            raster::draw_constrained_glyph_line(
                &mut current_image,
                self.view.image_width,
                self.view.image_height,
                &self.font,
                Scale::uniform(12f32),
                (pixel_x, pixel_y),
                4f32,
                &complex_str,
            );

            frame.data_mut(0).copy_from_slice(&current_image);

            self.media_out.write_frame(&frame)?;

            // call the progress callback every now and then
            let now = Instant::now();
            if now.saturating_duration_since(previous_progress) > self.video_progress_interval {
                self.video_progress_callback(frame_num);
                previous_progress = now;
            }

            frame_num += 1;
        }

        Ok(())
    }

    /// Renders the video as a Julia set following the specified path along the
    /// Mandelbrot set.
    fn render_julia(&mut self) -> Result<(), ApplicationRunError> {
        let mut frame = frame::Video::new(
            format::Pixel::RGBA,
            self.view.image_width,
            self.view.image_height,
        );
        let mut frame_num = 0;
        let mut previous_progress = Instant::now();

        let points =
            path_util::path_points(self.path.as_slice(), self.path_tolerance, self.step_length);

        for position in points {
            frame.set_pts(Some(frame_num as i64));

            let generator = generator::ValueGenerator::new(
                self.view,
                false,
                self.iterations,
                self.smoothing,
                Complex::<f64>::new(position.x as f64, position.y as f64),
            );

            let julia_image = generator::generate_fractal(
                &generator,
                num_cpus::get() + 2,
                |progress| self.fractal_progress_callback(progress),
                self.fractal_progress_interval,
            )?;

            frame.data_mut(0).copy_from_slice(&julia_image);

            self.media_out.write_frame(&frame)?;

            // call the progress callback every now and then
            let now = Instant::now();
            if now.saturating_duration_since(previous_progress) > self.video_progress_interval {
                self.video_progress_callback(frame_num);
                previous_progress = now;
            }

            frame_num += 1;
        }

        Ok(())
    }

    fn fractal_progress_callback(&self, progress: Vec<f32>) {
        println!("Fractal Generation Progress:");
        print!(" ");
        for f in progress {
            print!(" {:.2}%", f * 100f32);
        }
        println!();
    }

    fn video_progress_callback(&self, frame_num: u32) {
        println!("Generated {} frames out of {}", frame_num, self.frames);
    }
}

#[derive(Debug, Clone)]
enum ApplicationCreationError {
    MediaOutputCreationError(output::MediaOutputCreationError),
}

impl From<output::MediaOutputCreationError> for ApplicationCreationError {
    fn from(e: output::MediaOutputCreationError) -> Self {
        ApplicationCreationError::MediaOutputCreationError(e)
    }
}

#[derive(Debug, Clone)]
enum ApplicationRunError {
    FractalGenerationError(generator::FractalGenerationError),
    MediaWriteError(output::MediaWriteError),
}

impl From<generator::FractalGenerationError> for ApplicationRunError {
    fn from(e: generator::FractalGenerationError) -> Self {
        ApplicationRunError::FractalGenerationError(e)
    }
}

impl From<output::MediaWriteError> for ApplicationRunError {
    fn from(e: output::MediaWriteError) -> Self {
        ApplicationRunError::MediaWriteError(e)
    }
}
