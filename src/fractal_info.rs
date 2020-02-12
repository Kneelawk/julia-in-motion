use num_complex::Complex;

#[derive(Copy, Clone)]
pub struct FractalInfo {
    pub image_width: u32,
    pub image_height: u32,
    pub plane_width: f64,
    pub plane_height: f64,
    pub image_scale: f64,
    pub plane_start_x: f64,
    pub plane_start_y: f64,
    pub iterations: u32,
}

impl FractalInfo {
    pub fn new(
        image_width: u32,
        image_height: u32,
        plane_width: f64,
        iterations: u32,
    ) -> FractalInfo {
        let image_scale = plane_width / image_width as f64;
        let plane_height = image_height as f64 * image_scale;

        FractalInfo {
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConstrainedValue<T> {
    LessThanConstraint,
    WithinConstraint(T),
    GreaterThanConstraint,
}
