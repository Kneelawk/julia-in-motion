#![feature(try_trait)]

use crate::raster::ConstrainedValue;
use ffmpeg4::{format, frame};
use lyon_algorithms::{walk, walk::RegularPattern};
use lyon_path::iterator::PathIterator;
use num_complex::Complex;
use rusttype::{Font, Scale};
use std::time::{Duration, Instant};

mod args;
mod generator;
mod output;
mod path_length;
mod raster;
mod util;

macro_rules! path_walk_try {
    ($expr:expr, $error:ident) => {
        match $expr {
            Ok(value) => value,
            Err(err) => {
                $error = Some(err.into());
                return false;
            }
        }
    };
}

const FONT_DATA: &[u8] = include_bytes!("OxygenMono-Regular.ttf");

fn main() {
    let cmd_args = args::CmdArgs::load().expect("Error parsing commandline args");

    let font = Font::from_bytes(FONT_DATA).expect("Error loading font");

    let mut app = Application::new(cmd_args, font).expect("Error creating the application");

    app.run().expect("Error running the application");
}

struct Application<'a> {
    image_width: u32,
    image_height: u32,
    image_scale: f64,
    plane_start_x: f64,
    plane_start_y: f64,
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
        let image_scale = args.plane_width / args.image_width as f64;
        let plane_height = args.image_height as f64 * image_scale;

        // open the media output
        let media_out = output::MediaOutput::new(
            &args.output,
            args.image_width,
            args.image_height,
            args.time_base,
        )?;

        // walk along the path to determine its length
        let path_length =
            path_length::approximate_path_length(args.path.as_slice(), args.path_tolerance);

        // get the length of each step
        let step_length = path_length / args.frames as f32;

        Ok(Application {
            image_width: args.image_width,
            image_height: args.image_height,
            image_scale,
            plane_start_x: -args.plane_width / 2f64,
            plane_start_y: -plane_height / 2f64,
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
            self.image_scale,
            self.image_scale,
            self.plane_start_x,
            self.plane_start_y,
            true,
            self.iterations,
            self.smoothing,
            Complex::<f64>::new(0f64, 0f64),
        );

        let mandelbrot_image = generator::generate_fractal(
            &generator,
            self.image_width,
            self.image_height,
            num_cpus::get() + 2,
            |progress| self.fractal_progress_callback(progress),
            self.fractal_progress_interval,
        )?;

        let mut frame = frame::Video::new(format::Pixel::RGBA, self.image_width, self.image_height);
        let mut frame_num = 0;
        let mut previous_progress = Instant::now();
        let mut error = None;

        let step_length = self.step_length;
        let path = self.path.clone();
        let path_tolerance = self.path_tolerance;

        let mut pattern = RegularPattern {
            callback: &mut |position: lyon_algorithms::math::Point, _, _| {
                frame.set_pts(Some(frame_num as i64));
                let mut current_image = mandelbrot_image.clone();

                let complex = Complex::<f64>::new(position.x as f64, position.y as f64);
                let (pixel_x, pixel_y) = self.get_pixel_coordinates(complex);

                raster::draw_constrained_crosshair(
                    &mut current_image,
                    self.image_width,
                    self.image_height,
                    (pixel_x, pixel_y),
                );

                let complex_str = format!("{:.5} + {:.5}i", complex.re, complex.im);
                raster::draw_constrained_glyph_line(
                    &mut current_image,
                    self.image_width,
                    self.image_height,
                    &self.font,
                    Scale::uniform(12f32),
                    (pixel_x, pixel_y),
                    4f32,
                    &complex_str,
                );

                frame.data_mut(0).copy_from_slice(&current_image);

                path_walk_try!(self.media_out.write_frame(&frame), error);

                // call the progress callback every now and then
                let now = Instant::now();
                if now.saturating_duration_since(previous_progress) > self.video_progress_interval {
                    self.video_progress_callback(frame_num);
                    previous_progress = now;
                }

                frame_num += 1;

                error.is_none()
            },
            interval: step_length,
        };

        walk::walk_along_path(path.iter().flattened(path_tolerance), 0f32, &mut pattern);

        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Renders the video as a Julia set following the specified path along the
    /// Mandelbrot set.
    fn render_julia(&mut self) -> Result<(), ApplicationRunError> {
        let mut frame = frame::Video::new(format::Pixel::RGBA, self.image_width, self.image_height);
        let mut frame_num = 0;
        let mut previous_progress = Instant::now();
        let mut error = None;

        let step_length = self.step_length;
        let path = self.path.clone();
        let path_tolerance = self.path_tolerance;

        let mut pattern = RegularPattern {
            callback: &mut |position: lyon_algorithms::math::Point, _, _| {
                frame.set_pts(Some(frame_num as i64));

                let generator = generator::ValueGenerator::new(
                    self.image_scale,
                    self.image_scale,
                    self.plane_start_x,
                    self.plane_start_y,
                    false,
                    self.iterations,
                    self.smoothing,
                    Complex::<f64>::new(position.x as f64, position.y as f64),
                );

                let julia_image = path_walk_try!(
                    generator::generate_fractal(
                        &generator,
                        self.image_width,
                        self.image_height,
                        num_cpus::get() + 2,
                        |progress| self.fractal_progress_callback(progress),
                        self.fractal_progress_interval,
                    ),
                    error
                );

                frame.data_mut(0).copy_from_slice(&julia_image);

                path_walk_try!(self.media_out.write_frame(&frame), error);

                // call the progress callback every now and then
                let now = Instant::now();
                if now.saturating_duration_since(previous_progress) > self.video_progress_interval {
                    self.video_progress_callback(frame_num);
                    previous_progress = now;
                }

                frame_num += 1;

                error.is_none()
            },
            interval: step_length,
        };

        walk::walk_along_path(path.iter().flattened(path_tolerance), 0f32, &mut pattern);

        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
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

    fn get_pixel_coordinates(
        &self,
        plane_coordinates: Complex<f64>,
    ) -> (ConstrainedValue<u32>, ConstrainedValue<u32>) {
        (
            if plane_coordinates.re > self.plane_start_x {
                let x = ((plane_coordinates.re - self.plane_start_x) / self.image_scale) as u32;

                if x < self.image_width {
                    ConstrainedValue::WithinConstraint(x)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
            if plane_coordinates.im > self.plane_start_y {
                let y = ((plane_coordinates.im - self.plane_start_y) / self.image_scale) as u32;

                if y < self.image_height {
                    ConstrainedValue::WithinConstraint(y)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
        )
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
