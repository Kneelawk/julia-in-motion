use ffmpeg4::Rational;
use regex::Regex;
use std::num::ParseIntError;

lazy_static::lazy_static! {
    static ref RATIONAL_REGEX: Regex = Regex::new(r"^(\d+)/(\d+)$").unwrap();
}

pub fn parse_rational(string: &str) -> Result<Rational, ParseRationalError> {
    if let Some(captures) = RATIONAL_REGEX.captures(string) {
        Ok(Rational::new(
            captures[1].parse::<i32>()?,
            captures[2].parse::<i32>()?,
        ))
    } else {
        Err(ParseRationalError::NotARational)
    }
}

#[derive(Debug, Clone)]
pub enum ParseRationalError {
    NotARational,
    InvalidRationalComponent(ParseIntError),
}

impl From<ParseIntError> for ParseRationalError {
    fn from(e: ParseIntError) -> Self {
        ParseRationalError::InvalidRationalComponent(e)
    }
}
