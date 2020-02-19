use args::Smoothing;
use num_complex::Complex;
use std::{
    intrinsics::transmute,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub mod args;
pub mod view;

#[derive(Debug, Clone)]
pub struct ValueGenerator {
    view: view::View,
    mandelbrot: bool,
    iterations: u32,
    smoothing: Smoothing,
    c: Complex<f64>,
}

pub struct FractalThread {
    name: String,
    progress: RwLock<f32>,
    state: RwLock<FractalThreadState>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FractalThreadState {
    NotStarted,
    Running,
    Finished,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RGBAColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FractalThreadMessage {
    index: usize,
    color: RGBAColor,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FractalGenerationError {}

pub fn generate_fractal<P: Fn(Vec<f32>)>(
    generator: &ValueGenerator,
    num_threads: usize,
    progress_callback: P,
    progress_interval: Duration,
) -> Result<Box<[u8]>, FractalGenerationError> {
    let width = generator.view.image_width;
    let height = generator.view.image_height;

    let mut threads = vec![];

    for i in 0..num_threads {
        threads.push(FractalThread::new(format!("Fractal Thread {}", i)));
    }

    let rx = {
        let (tx, rx) = channel();
        let left_over = width as usize * height as usize % num_threads;

        // start all the threads
        for (index, thread) in threads.iter().enumerate() {
            let chunk_height = width as usize * height as usize / num_threads
                + if index < left_over { 1 } else { 0 };
            thread.start_generation(
                tx.clone(),
                width,
                chunk_height,
                index,
                num_threads,
                &generator,
            );
        }

        rx
    };

    let mut image = vec![0u8; (width * height * 4) as usize].into_boxed_slice();

    let mut previous_progress = Instant::now();

    for message in rx {
        let FractalThreadMessage { index, color } = message;
        image[index * 4..index * 4 + 4].copy_from_slice(&Into::<[u8; 4]>::into(color));

        // send progress reports every now and then
        let now = Instant::now();
        if now.saturating_duration_since(previous_progress) > progress_interval {
            let mut thread_progress = vec![];
            for thread in threads.iter() {
                thread_progress.push(thread.get_progress());
            }

            progress_callback(thread_progress);

            previous_progress = now;
        }
    }

    Ok(image)
}

impl ValueGenerator {
    /// Creates a new ValueGenerator.
    pub fn new(
        view: view::View,
        mandelbrot: bool,
        iterations: u32,
        smoothing: Smoothing,
        c: Complex<f64>,
    ) -> ValueGenerator {
        ValueGenerator {
            view,
            mandelbrot,
            iterations,
            smoothing,
            c,
        }
    }

    /// Gets the value at a specific location on the fractal described by this
    /// ValueGenerator.
    pub fn gen_value(&self, loc: Complex<f64>) -> f64 {
        let (mut z, c): (Complex<f64>, Complex<f64>) = if self.mandelbrot {
            (Complex::<f64>::new(0f64, 0f64), loc)
        } else {
            (loc, self.c)
        };

        let mut z_prev = z;

        let radius_squared = self.smoothing.radius_squared();

        let mut n = 0;
        while n < self.iterations {
            if z.norm_sqr() > radius_squared {
                break;
            }

            z_prev = z;

            z = z * z + c;

            n += 1;
        }

        self.smoothing.smooth(n, z, z_prev)
    }

    pub fn gen_pixel_value(&self, x: u32, y: u32) -> f64 {
        self.gen_value(self.view.get_plane_coordinates((x, y)))
    }

    pub fn gen_color(&self, value: f64) -> RGBAColor {
        if value < self.iterations as f64 {
            RGBAColor::from_hsb(
                mod2(value * 3.3f64, 0f64, 256f64) / 256f64,
                1f64,
                mod2(value * 16f64, 0f64, 256f64) / 256f64,
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
        img_data: Sender<FractalThreadMessage>,
        chunk_width: u32,
        size: usize,
        offset: usize,
        skip: usize,
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
                    .spawn(move || {
                        clone.image_thread_func(
                            img_data,
                            chunk_width,
                            size,
                            offset,
                            skip,
                            generator,
                        )
                    })
                    .expect("Unable to spawn fractal thread"),
            );
        }
    }

    fn image_thread_func(
        &self,
        img_data: Sender<FractalThreadMessage>,
        chunk_width: u32,
        size: usize,
        offset: usize,
        skip: usize,
        generator: ValueGenerator,
    ) {
        for i in 0usize..size {
            let index = i * skip + offset;

            let x = (index % chunk_width as usize) as u32;
            let y = (index / chunk_width as usize) as u32;

            let color = generator.gen_pixel(x, y);
            img_data
                .send(FractalThreadMessage { index, color })
                .unwrap();

            *self.progress.write().unwrap() = (i + 1) as f32 / size as f32;
        }

        *self.state.write().unwrap() = FractalThreadState::Finished;
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
