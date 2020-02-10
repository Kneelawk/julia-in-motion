use ffmpeg4::{codec, encoder, format, frame, media, software, util, Packet, Rational};
use std::{option::NoneError, path::Path};

pub struct MediaOutput {
    format_context: format::context::Output,
    encoder: codec::encoder::Video,
    converter: software::scaling::Context,
    in_time_base: Rational,
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
        let codec_context =
            encoder::find(format_context.format().codec(path, media::Type::Video))?.video()?;

        let global_header = format_context
            .format()
            .flags()
            .contains(format::Flags::GLOBAL_HEADER);

        let mut output = format_context.add_stream(codec_context)?;
        let mut encoder = output.codec().encoder().video()?;

        if global_header {
            encoder.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        encoder.set_frame_rate(Some((30, 1)));
        encoder.set_format(format::Pixel::YUV420P);
        encoder.set_bit_rate(960000);
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_time_base(time_base);
        output.set_time_base(time_base);

        let encoder = encoder.open_as(codec_context)?;

        output.set_parameters(&encoder);

        let converter =
            software::converter((width, height), format::Pixel::RGBA, format::Pixel::YUV420P)?;

        Ok(MediaOutput {
            format_context,
            encoder,
            converter,
            in_time_base: time_base,
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
        let mut converted = frame::Video::empty();
        let mut encoded = Packet::empty();

        self.converter.run(frame, &mut converted)?;

        if self.encoder.encode(&converted, &mut encoded)? {
            encoded.set_pts(frame.pts());
            encoded.set_stream(0);
            encoded.rescale_ts(
                self.in_time_base,
                self.format_context.stream(0)?.time_base(),
            );
            encoded.write_interleaved(&mut self.format_context)?;

            Ok(MediaWriteResult::PacketWritten)
        } else {
            Ok(MediaWriteResult::NoPacketWritten)
        }
    }

    pub fn finish(&mut self) -> Result<MediaWriteResult, MediaWriteError> {
        let mut encoded = Packet::empty();

        let res = if self.encoder.flush(&mut encoded)? {
            encoded.set_stream(0);
            encoded.rescale_ts(
                self.in_time_base,
                self.format_context.stream(0)?.time_base(),
            );
            encoded.write_interleaved(&mut self.format_context)?;

            MediaWriteResult::PacketWritten
        } else {
            MediaWriteResult::NoPacketWritten
        };

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
