use ffmpeg4::{codec, encoder, format, frame, media, software, util, Packet, Rational};
use std::{option::NoneError, path::Path};

mod extra;

pub struct MediaOutput {
    format_context: format::context::Output,
    encoder: codec::encoder::Video,
    converter: software::scaling::Context,
    in_time_base: Rational,
    converted: frame::Video,
    encoded: Packet,
}

impl MediaOutput {
    pub fn new<P: AsRef<Path>, R: Into<Rational>>(
        path: &P,
        width: u32,
        height: u32,
        time_base: R,
    ) -> Result<MediaOutput, MediaOutputCreationError> {
        let time_base = time_base.into();
        let mut format_context = format::output(path)?;
        let codec =
            encoder::find(format_context.format().codec(path, media::Type::Video))?.video()?;

        let global_header = format_context
            .format()
            .flags()
            .contains(format::Flags::GLOBAL_HEADER);

        let mut output = format_context.add_stream(codec)?;
        let mut encoder = output.codec().encoder().video()?;

        if global_header {
            encoder.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        encoder.set_frame_rate(Some((30, 1)));
        encoder.set_format(format::Pixel::YUV420P);
        encoder.set_bit_rate(0);
        extra::codec_opt_set_str(&mut encoder, "crf", "30")?;
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_time_base(time_base);
        output.set_time_base(time_base);

        let encoder = encoder.open_as(codec)?;

        output.set_parameters(&encoder);

        let converter =
            software::converter((width, height), format::Pixel::RGBA, format::Pixel::YUV420P)?;

        Ok(MediaOutput {
            format_context,
            encoder,
            converter,
            in_time_base: time_base,
            converted: frame::Video::empty(),
            encoded: Packet::empty(),
        })
    }

    pub fn start(&mut self) -> Result<(), MediaWriteError> {
        self.format_context.write_header()?;

        Ok(())
    }

    pub fn write_frame(
        &mut self,
        frame: &frame::Video,
    ) -> Result<MediaWriteResult, MediaWriteError> {
        self.converter.run(frame, &mut self.converted)?;
        self.converted.set_pts(frame.pts());

        if self.encoder.encode(&self.converted, &mut self.encoded)? {
            self.encoded.set_stream(0);
            self.encoded.rescale_ts(
                self.in_time_base,
                self.format_context.stream(0)?.time_base(),
            );
            self.encoded.write_interleaved(&mut self.format_context)?;

            Ok(MediaWriteResult::PacketWritten)
        } else {
            Ok(MediaWriteResult::NoPacketWritten)
        }
    }

    pub fn finish(&mut self) -> Result<MediaWriteResult, MediaWriteError> {
        let mut res = MediaWriteResult::NoPacketWritten;

        // sometimes there are a bunch of unwritten frames
        while self.encoder.flush(&mut self.encoded)? {
            self.encoded.set_stream(0);
            self.encoded.rescale_ts(
                self.in_time_base,
                self.format_context.stream(0)?.time_base(),
            );
            self.encoded.write_interleaved(&mut self.format_context)?;

            res = MediaWriteResult::PacketWritten;
        }

        self.format_context.write_trailer()?;

        Ok(res)
    }
}

#[derive(Clone, Debug)]
pub enum MediaOutputCreationError {
    FfmpegError(util::error::Error),
    MissingComponentError,
}

impl From<util::error::Error> for MediaOutputCreationError {
    fn from(e: util::error::Error) -> Self {
        MediaOutputCreationError::FfmpegError(e)
    }
}

impl From<NoneError> for MediaOutputCreationError {
    fn from(_e: NoneError) -> Self {
        MediaOutputCreationError::MissingComponentError
    }
}

#[derive(Clone, Debug)]
pub enum MediaWriteError {
    FfmpegError(util::error::Error),
    MissingComponentError,
}

impl From<util::error::Error> for MediaWriteError {
    fn from(e: util::error::Error) -> Self {
        MediaWriteError::FfmpegError(e)
    }
}

impl From<NoneError> for MediaWriteError {
    fn from(_e: NoneError) -> Self {
        MediaWriteError::MissingComponentError
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaWriteResult {
    PacketWritten,
    NoPacketWritten,
}
