use num_complex::Complex;
use std::{
    intrinsics::transmute,
    mem::replace,
    sync::{Arc, Mutex, RwLock},
    thread,
    thread::{spawn, JoinHandle},
    time::Duration,
};

const WAIT_DURATION: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct ValueGenerator {
    img_scale_x: f64,
    img_scale_y: f64,
    plane_zero_x: f64,
    plane_zero_y: f64,
    mandelbrot: bool,
    iterations: u32,
    c: Complex<f64>,
}

pub struct FractalThread {
    name: String,
    progress: RwLock<f32>,
    state: RwLock<FractalThreadState>,
    thread: Mutex<Option<JoinHandle<Box<[u8]>>>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FractalThreadState {
    NotStarted,
    Running,
    Finished,
    Error,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FractalThreadError {
    NoResult,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RGBAColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FractalGenerationError {}

pub fn generate_fractal<P: Fn(Vec<f32>)>(
    generator: &ValueGenerator,
    width: u32,
    height: u32,
    num_threads: usize,
    progress_callback: P,
) -> Result<Box<[u8]>, FractalGenerationError> {
    let mut threads = vec![];

    for i in 0..num_threads {
        threads.push(FractalThread::new(format!("Fractal Thread {}", i)));
    }

    let left_over = height as usize % num_threads;

    // start all the threads
    let mut offset = 0;
    for (index, thread) in threads.iter().enumerate() {
        let mut sub_generator = generator.clone();
        sub_generator.plane_zero_y += offset as f64 * generator.img_scale_y;
        let chunk_height = height as usize / num_threads + if index < left_over { 1 } else { 0 };
        thread.start_generation(width, chunk_height * width as usize, &sub_generator);
        offset += chunk_height;
    }

    let mut running = true;

    while running {
        running = false;
        let mut thread_progress = vec![];

        for thread in threads.iter() {
            thread_progress.push(thread.get_progress());

            if thread.get_state() == FractalThreadState::Running {
                running = true;
            }
        }

        progress_callback(thread_progress);

        thread::sleep(WAIT_DURATION);
    }

    let mut image = vec![0u8; (width * height * 4) as usize].into_boxed_slice();

    let mut offset = 0usize;
    for thread in threads.iter() {
        let chunk = thread.generation_result().unwrap();
        image[offset..(offset + chunk.len())].copy_from_slice(&chunk);
        offset += chunk.len();
    }

    Ok(image)
}

impl ValueGenerator {
    /// Creates a new ValueGenerator.
    pub fn new(
        img_scale_x: f64,
        img_scale_y: f64,
        plane_zero_x: f64,
        plane_zero_y: f64,
        mandelbrot: bool,
        iterations: u32,
        c: Complex<f64>,
    ) -> ValueGenerator {
        ValueGenerator {
            img_scale_x,
            img_scale_y,
            plane_zero_x,
            plane_zero_y,
            mandelbrot,
            iterations,
            c,
        }
    }

    /// Gets the value at a specific location on the fractal described by this
    /// ValueGenerator.
    pub fn gen_value(&self, loc: Complex<f64>) -> u32 {
        let (mut z, c) = if self.mandelbrot {
            (Complex::<f64>::new(0f64, 0f64), loc)
        } else {
            (loc, self.c)
        };

        let mut n = 0;
        while n < self.iterations {
            if z.norm_sqr() > 1f64 {
                break;
            }

            z = z * z + c;

            n += 1;
        }
        n
    }

    pub fn gen_pixel_value(&self, x: u32, y: u32) -> u32 {
        self.gen_value(Complex::<f64>::new(
            x as f64 * self.img_scale_x + self.plane_zero_x,
            y as f64 * self.img_scale_y + self.plane_zero_y,
        ))
    }

    pub fn gen_color(&self, value: u32) -> RGBAColor {
        if value < self.iterations {
            RGBAColor::from_hsb(
                mod2(value as f64 * 3.3f64, 0f64, 256f64) / 256f64,
                1f64,
                mod2(value as f64 * 16f64, 0f64, 256f64) / 256f64,
                1f64,
            )
        } else {
            RGBAColor::new(0, 0, 0, 255)
        }
    }

    pub fn gen_pixel(&self, x: u32, y: u32) -> RGBAColor {
        self.gen_color(self.gen_pixel_value(x, y))
    }
}

impl FractalThread {
    pub fn new(name: String) -> Arc<FractalThread> {
        Arc::new(FractalThread {
            name,
            progress: RwLock::new(0f32),
            state: RwLock::new(FractalThreadState::NotStarted),
            thread: Mutex::new(None),
        })
    }

    pub fn start_generation(
        self: &Arc<Self>,
        chunk_width: u32,
        size: usize,
        generator: &ValueGenerator,
    ) {
        let mut state = self.state.write().unwrap();
        if *state != FractalThreadState::Running {
            *state = FractalThreadState::Running;
            *self.progress.write().unwrap() = 0f32;
            let clone = self.clone();
            let generator = generator.clone();
            *self.thread.lock().unwrap() = Some(
                thread::Builder::new()
                    .name(self.name.clone())
                    .spawn(move || clone.image_thread_func(chunk_width, size, generator))
                    .expect("Unable to spawn fractal thread"),
            );
        }
    }

    pub fn generation_result(&self) -> Result<Box<[u8]>, FractalThreadError> {
        let mut thread = self.thread.lock().unwrap();
        if thread.is_some() {
            let thread = replace(&mut *thread, None);
            Ok(thread.unwrap().join().unwrap())
        } else {
            Err(FractalThreadError::NoResult)
        }
    }

    fn image_thread_func(
        &self,
        chunk_width: u32,
        size: usize,
        generator: ValueGenerator,
    ) -> Box<[u8]> {
        let mut img_data = vec![0; size * 4].into_boxed_slice();

        for i in 0usize..size {
            let x = (i % chunk_width as usize) as u32;
            let y = (i / chunk_width as usize) as u32;

            let color = generator.gen_pixel(x, y);
            img_data[(i * 4)..(i * 4 + 4)].copy_from_slice(&Into::<[u8; 4]>::into(color));

            *self.progress.write().unwrap() = (i + 1) as f32 / size as f32;
        }

        *self.state.write().unwrap() = FractalThreadState::Finished;

        img_data
    }

    pub fn get_progress(&self) -> f32 {
        *self.progress.read().unwrap()
    }

    pub fn get_state(&self) -> FractalThreadState {
        *self.state.read().unwrap()
    }
}

impl RGBAColor {
    /// Creates a new RGBAColor from the given color byte values.
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8) -> RGBAColor {
        RGBAColor {
            r: red,
            g: green,
            b: blue,
            a: alpha,
        }
    }

    /// Creates a new RGBAColor from these HSBA values. All HSBA values must be
    /// in the range 0..1.
    pub fn from_hsb(hue: f64, saturation: f64, brightness: f64, alpha: f64) -> RGBAColor {
        let alpha = (alpha * 255f64 + 0.5f64) as u8;
        if saturation == 0f64 {
            let brightness = (brightness * 255f64 + 0.5f64) as u8;
            RGBAColor {
                r: brightness,
                g: brightness,
                b: brightness,
                a: alpha,
            }
        } else {
            let sector = (hue - hue.floor()) * 6f64;
            let offset_in_sector = sector - sector.floor();
            let off = brightness * (1f64 - saturation);
            let fade_out = brightness * (1f64 - saturation * offset_in_sector);
            let fade_in = brightness * (1f64 - saturation * (1f64 - offset_in_sector));
            match sector as u32 {
                0 => RGBAColor {
                    r: (brightness * 255f64 + 0.5f64) as u8,
                    g: (fade_in * 255f64 + 0.5f64) as u8,
                    b: (off * 255f64 + 0.5f64) as u8,
                    a: alpha,
                },
                1 => RGBAColor {
                    r: (fade_out * 255f64 + 0.5f64) as u8,
                    g: (brightness * 255f64 + 0.5f64) as u8,
                    b: (off * 255f64 + 0.5f64) as u8,
                    a: alpha,
                },
                2 => RGBAColor {
                    r: (off * 255f64 + 0.5f64) as u8,
                    g: (brightness * 255f64 + 0.5f64) as u8,
                    b: (fade_in * 255f64 + 0.5f64) as u8,
                    a: alpha,
                },
                3 => RGBAColor {
                    r: (off * 255f64 + 0.5f64) as u8,
                    g: (fade_out * 255f64 + 0.5f64) as u8,
                    b: (brightness * 255f64 + 0.5f64) as u8,
                    a: alpha,
                },
                4 => RGBAColor {
                    r: (fade_in * 255f64 + 0.5f64) as u8,
                    g: (off * 255f64 + 0.5f64) as u8,
                    b: (brightness * 255f64 + 0.5f64) as u8,
                    a: alpha,
                },
                5 => RGBAColor {
                    r: (brightness * 255f64 + 0.5f64) as u8,
                    g: (off * 255f64 + 0.5f64) as u8,
                    b: (fade_out * 255f64 + 0.5f64) as u8,
                    a: alpha,
                },
                _ => unreachable!("Invalid color wheel sector"),
            }
        }
    }
}

impl Into<[u8; 4]> for RGBAColor {
    fn into(self) -> [u8; 4] {
        unsafe { transmute(self) }
    }
}

fn mod2(mut value: f64, min: f64, max: f64) -> f64 {
    let size = max - min;

    while value < min {
        value += size;
    }
    while value >= max {
        value -= size;
    }

    value
}
