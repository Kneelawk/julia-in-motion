use crate::generator::view::ConstrainedValue;
use rusttype::{Font, Scale};

// Draws a crosshair at the specified pixel location if within the constraint.
pub fn draw_constrained_crosshair(
    image: &mut [u8],
    image_width: u32,
    image_height: u32,
    (pixel_x, pixel_y): (ConstrainedValue<u32>, ConstrainedValue<u32>),
) {
    if let ConstrainedValue::WithinConstraint(pixel_y) = pixel_y {
        draw_horizontal_line(image, image_width, pixel_y);
    }
    if let ConstrainedValue::WithinConstraint(pixel_x) = pixel_x {
        draw_vertical_line(image, image_width, image_height, pixel_x);
    }
}

/// Draws a vertical line across the image at the specified x coordinate.
pub fn draw_vertical_line(image: &mut [u8], image_width: u32, image_height: u32, pixel_x: u32) {
    for y in 0..image_height as usize {
        let index = (y * image_width as usize + pixel_x as usize) * 4;
        image[index] = 0xFFu8;
        image[index + 1] = 0xFFu8;
        image[index + 2] = 0xFFu8;
        image[index + 3] = 0xFFu8;
    }
}

/// Draws a horizontal line across the image at the specified y coordinate.
pub fn draw_horizontal_line(image: &mut [u8], image_width: u32, pixel_y: u32) {
    for x in 0..image_width as usize {
        let index = (pixel_y as usize * image_width as usize + x) * 4;
        image[index] = 0xFFu8;
        image[index + 1] = 0xFFu8;
        image[index + 2] = 0xFFu8;
        image[index + 3] = 0xFFu8;
    }
}

/// Draws a string of glyphs at a constrained pixel location, making sure the
/// string is closest to the center of the image.
pub fn draw_constrained_glyph_line(
    image: &mut [u8],
    image_width: u32,
    image_height: u32,
    font: &Font,
    scale: Scale,
    (x, y): (ConstrainedValue<u32>, ConstrainedValue<u32>),
    margin: f32,
    string: &str,
) {
    let (line_width, line_height) = get_glyph_line_dimensions(font, scale, margin, string);

    let x = match x {
        ConstrainedValue::LessThanConstraint => 0,
        ConstrainedValue::WithinConstraint(v) => {
            if v < image_width / 2 {
                v
            } else {
                v - line_width as u32
            }
        }
        ConstrainedValue::GreaterThanConstraint => image_width - line_width as u32,
    };
    let y = match y {
        ConstrainedValue::LessThanConstraint => 0,
        ConstrainedValue::WithinConstraint(v) => {
            if v < image_height / 2 {
                v
            } else {
                v - line_height as u32
            }
        }
        ConstrainedValue::GreaterThanConstraint => image_height - line_height as u32,
    };

    draw_glyph_line(
        image,
        image_width,
        image_height,
        font,
        scale,
        (x, y),
        margin,
        string,
    );
}

/// Draws a string of glyphs in a line (left to right) onto the image buffer.
pub fn draw_glyph_line(
    image: &mut [u8],
    image_width: u32,
    image_height: u32,
    font: &Font,
    scale: Scale,
    (x, y): (u32, u32),
    margin: f32,
    string: &str,
) {
    let ascent = font.v_metrics(scale).ascent;

    for glyph in font.layout(
        string,
        scale,
        rusttype::point(x as f32 + margin, y as f32 + margin + ascent),
    ) {
        if let Some(bounding_box) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, c| {
                let pixel_x = x + bounding_box.min.x as u32;
                let pixel_y = y + bounding_box.min.y as u32;
                if pixel_x < image_width && pixel_y < image_height {
                    let index = ((pixel_y * image_width + pixel_x) * 4) as usize;
                    let value = (255f32 * c) as u8;
                    let back = 1f32 - c;
                    image[index] = value + (back * image[index] as f32) as u8;
                    image[index + 1] = value + (back * image[index + 1] as f32) as u8;
                    image[index + 2] = value + (back * image[index + 2] as f32) as u8;
                    image[index + 3] = value + (back * image[index + 3] as f32) as u8;
                }
            });
        }
    }
}

/// Gets the dimensions of a single line of glyphs
pub fn get_glyph_line_dimensions(
    font: &Font,
    scale: Scale,
    margin: f32,
    string: &str,
) -> (f32, f32) {
    let str_len = string.len();
    let mut width = margin * 2f32;
    let mut last = None;

    for (index, glyph) in font.glyphs_for(string.chars()).enumerate() {
        let glyph = glyph.scaled(scale);
        if let Some(last) = last {
            width += font.pair_kerning(scale, last, glyph.id());
        }
        if index < str_len - 1 {
            width += glyph.h_metrics().advance_width;
        } else {
            width += glyph.h_metrics().left_side_bearing;
        }
        last = Some(glyph.id());
    }

    let v_metrics = font.v_metrics(scale);

    (width, v_metrics.ascent - v_metrics.descent + margin * 2f32)
}
