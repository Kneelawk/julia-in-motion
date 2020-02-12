use lyon_path::{iterator::PathIterator, Event, PathSlice};

// Approximates the length of a path given a tolerance.
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
