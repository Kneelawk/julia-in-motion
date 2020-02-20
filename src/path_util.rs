use lyon_algorithms::walk::{walk_along_path, RegularPattern};
use lyon_path::{iterator::PathIterator, math::Point, Event, PathSlice};

/// Approximates the length of a path given a tolerance.
pub fn approximate_path_length(path: PathSlice, tolerance: f32) -> f32 {
    // More or less copied from https://github.com/nical/lyon/blob/cb23ba4a527b2f246ec54a0cfde01f062f2b5159/path/src/iterator.rs#L706

    let mut length = 0f32;
    for event in path.iter().flattened(tolerance) {
        match event {
            Event::Begin { .. } => {}
            Event::Line { from, to } => {
                length += (to - from).length();
            }
            Event::Quadratic { .. } => {}
            Event::Cubic { .. } => {}
            Event::End { last, first, close } => {
                if close {
                    length += (first - last).length();
                }
            }
        }
    }

    length
}

/// Walks along a path and returns a vector of points at regular intervals.
pub fn path_points(path: PathSlice, curve_tolerance: f32, interval: f32) -> Vec<Point> {
    let mut points = vec![];

    let mut pattern = RegularPattern {
        callback: &mut |point: Point, _, _| {
            points.push(point);

            true
        },
        interval,
    };

    walk_along_path(path.iter().flattened(curve_tolerance), 0f32, &mut pattern);

    points
}
