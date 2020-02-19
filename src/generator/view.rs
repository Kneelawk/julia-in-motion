use num_complex::Complex;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct View {
    pub image_width: u32,
    pub image_height: u32,
    pub image_scale_x: f64,
    pub image_scale_y: f64,
    pub plane_start_x: f64,
    pub plane_start_y: f64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConstrainedValue<T> {
    LessThanConstraint,
    WithinConstraint(T),
    GreaterThanConstraint,
}

impl View {
    pub fn new_uniform(image_width: u32, image_height: u32, plane_width: f64) -> View {
        let image_scale = plane_width / image_width as f64;
        let plane_height = image_height as f64 * image_scale;

        View {
            image_width,
            image_height,
            image_scale_x: image_scale,
            image_scale_y: image_scale,
            plane_start_x: -plane_width / 2f64,
            plane_start_y: -plane_height / 2f64,
        }
    }

    pub fn get_plane_coordinates(&self, (x, y): (u32, u32)) -> Complex<f64> {
        Complex::<f64>::new(
            x as f64 * self.image_scale_x + self.plane_start_x,
            y as f64 * self.image_scale_y + self.plane_start_y,
        )
    }

    pub fn get_pixel_coordinates(
        &self,
        plane_coordinates: Complex<f64>,
    ) -> (ConstrainedValue<u32>, ConstrainedValue<u32>) {
        (
            if plane_coordinates.re > self.plane_start_x {
                let x = ((plane_coordinates.re - self.plane_start_x) / self.image_scale_x) as u32;

                if x < self.image_width {
                    ConstrainedValue::WithinConstraint(x)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
            if plane_coordinates.im > self.plane_start_y {
                let y = ((plane_coordinates.im - self.plane_start_y) / self.image_scale_y) as u32;

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
