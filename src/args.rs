use crate::{generator, util};
use ffmpeg4::Rational;
use std::{
    fmt::{Display, Error, Formatter},
    fs::create_dir_all,
    io,
    num::{ParseFloatError, ParseIntError},
    path::{Path, PathBuf},
    time::Duration,
};

pub struct CmdArgs {
    pub image_width: u32,
    pub image_height: u32,
    pub plane_width: f64,
    pub frames: u32,
    pub path: lyon_path::Path,
    pub output: PathBuf,
    pub iterations: u32,
    pub fractal_progress_interval: Duration,
    pub video_progress_interval: Duration,
    pub time_base: Rational,
    pub path_tolerance: f32,
    pub smoothing: generator::args::Smoothing,
    pub mandelbrot: bool,
}

impl CmdArgs {
    pub fn load() -> Result<CmdArgs, CmdArgsLoadError> {
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
            .map_err(|e| CmdArgsLoadError::from_int("image-width", e))?;
        let image_height = matches
            .value_of("image_height")
            .unwrap()
            .parse::<u32>()
            .map_err(|e| CmdArgsLoadError::from_int("image-height", e))?;
        let frames = matches
            .value_of("frames")
            .unwrap()
            .parse::<u32>()
            .map_err(|e| CmdArgsLoadError::from_int("frames", e))?;
        let plane_width = matches
            .value_of("plane_width")
            .unwrap()
            .parse::<f64>()
            .map_err(|e| CmdArgsLoadError::from_float("plane-width", e))?;

        // parse the output file and create its parent directories if needed
        let output = Path::new(matches.value_of("output").unwrap());
        if let Some(parent) = output.parent() {
            if !parent.exists() {
                create_dir_all(parent)?;
            }
        }

        // parse the path string as an SVG path
        let path_str = matches.value_of("path").unwrap();
        let svg_builder = lyon_path::Path::builder().with_svg();
        let path = lyon_svg::path_utils::build_path(svg_builder, path_str)
            .map_err(|e| CmdArgsLoadError::from_path("path", e))?;

        // get the optional arguments
        let iterations = matches
            .value_of("iterations")
            .unwrap()
            .parse::<u32>()
            .map_err(|e| CmdArgsLoadError::from_int("iterations", e))?;
        let fractal_progress_interval = Duration::from_millis(
            matches
                .value_of("fractal_progress_interval")
                .unwrap()
                .parse::<u64>()
                .map_err(|e| CmdArgsLoadError::from_int("fractal-progress-interval", e))?,
        );
        let video_progress_interval = Duration::from_millis(
            matches
                .value_of("video_progress_interval")
                .unwrap()
                .parse::<u64>()
                .map_err(|e| CmdArgsLoadError::from_int("video-progress-interval", e))?,
        );
        let time_base = util::parse_rational(matches.value_of("time_base").unwrap())
            .map_err(|e| CmdArgsLoadError::from_rational("time-base", e))?;

        // get the path tolerance
        let path_tolerance = matches
            .value_of("path_tolerance")
            .unwrap()
            .parse::<f32>()
            .map_err(|e| CmdArgsLoadError::from_float("path-tolerance", e))?;

        // get the kind of smoothing to use
        let smoothing = matches
            .value_of("smoothing")
            .unwrap()
            .parse::<generator::args::Smoothing>()
            .map_err(|e| CmdArgsLoadError::from_smoothing("smoothing", e))?;

        // get the flags
        let mandelbrot = matches.is_present("mandelbrot");

        Ok(CmdArgs {
            image_width,
            image_height,
            plane_width,
            frames,
            path,
            output: output.to_path_buf(),
            iterations,
            fractal_progress_interval,
            video_progress_interval,
            time_base,
            path_tolerance,
            smoothing,
            mandelbrot,
        })
    }
}

#[derive(Debug)]
pub enum CmdArgsLoadError {
    IOError(io::Error),
    ParseError {
        argument: String,
        cause: ParseErrorCause,
    },
}

#[derive(Debug, Clone)]
pub enum ParseErrorCause {
    ParseFloatError(ParseFloatError),
    ParseIntError(ParseIntError),
    ParsePathError(lyon_svg::path_utils::ParseError),
    ParseRationalError(util::ParseRationalError),
    ParseSmoothingError(generator::args::ParseSmoothingError),
}

impl CmdArgsLoadError {
    pub fn from_float(argument: &str, error: ParseFloatError) -> CmdArgsLoadError {
        CmdArgsLoadError::ParseError {
            argument: argument.to_owned(),
            cause: ParseErrorCause::ParseFloatError(error),
        }
    }

    pub fn from_int(argument: &str, error: ParseIntError) -> CmdArgsLoadError {
        CmdArgsLoadError::ParseError {
            argument: argument.to_owned(),
            cause: ParseErrorCause::ParseIntError(error),
        }
    }

    pub fn from_path(argument: &str, error: lyon_svg::path_utils::ParseError) -> CmdArgsLoadError {
        CmdArgsLoadError::ParseError {
            argument: argument.to_owned(),
            cause: ParseErrorCause::ParsePathError(error),
        }
    }

    pub fn from_rational(argument: &str, error: util::ParseRationalError) -> CmdArgsLoadError {
        CmdArgsLoadError::ParseError {
            argument: argument.to_owned(),
            cause: ParseErrorCause::ParseRationalError(error),
        }
    }

    pub fn from_smoothing(
        argument: &str,
        error: generator::args::ParseSmoothingError,
    ) -> CmdArgsLoadError {
        CmdArgsLoadError::ParseError {
            argument: argument.to_owned(),
            cause: ParseErrorCause::ParseSmoothingError(error),
        }
    }
}

impl Display for CmdArgsLoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            CmdArgsLoadError::ParseError { argument, .. } => {
                f.write_fmt(format_args!("Unable to parse --{} argument", argument))
            }
            CmdArgsLoadError::IOError(_) => f.write_str("IO Error"),
        }
    }
}

impl From<io::Error> for CmdArgsLoadError {
    fn from(e: io::Error) -> Self {
        CmdArgsLoadError::IOError(e)
    }
}
