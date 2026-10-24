//! Decoded clipboard PNGs for the palette: 52 DIP row thumbnails and the
//! preview pane's fitted image.
//!
//! Clipboard captures are full screenshots, so each image is downscaled once
//! to the pixels its target actually shows and then kept for the current file
//! version. The list and the preview ask for different sizes; both share this
//! cache, which is capped so scrolling a long image history cannot grow GPU
//! memory without bound.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// Decoded RGBA kept for the visible images. Preview images are about 1 MB
/// each at 125% DPI; thumbnails are a few KB.
const CACHE_BUDGET_BYTES: usize = 48 * 1024 * 1024;
const CACHE_MAX_IMAGES: usize = 32;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    path: String,
    mtime: i64,
    width: u32,
    height: u32,
}

struct Cached {
    image: Image,
    bytes: usize,
}

#[derive(Default)]
pub struct ClipImages {
    map: HashMap<Key, Option<Cached>>,
    bytes: usize,
}

impl ClipImages {
    pub fn new() -> Self {
        Self::default()
    }

    /// The image at `path`, fitted inside `max_w` by `max_h` physical pixels.
    /// Decode failures are cached until the file changes, so a broken capture
    /// is not retried on every scroll or keystroke.
    pub fn image(&mut self, path: &str, max_w: u32, max_h: u32) -> Option<Image> {
        if path.is_empty() || max_w == 0 || max_h == 0 {
            return None;
        }
        let key = Key {
            path: path.to_string(),
            mtime: file_mtime(path),
            width: max_w,
            height: max_h,
        };
        if let Some(hit) = self.map.get(&key) {
            return hit.as_ref().map(|cached| cached.image.clone());
        }
        let cached = decode(path, max_w, max_h).map(|(width, height, rgba)| {
            let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
            for (dst, src) in buffer.make_mut_slice().iter_mut().zip(rgba.chunks_exact(4)) {
                dst.r = src[0];
                dst.g = src[1];
                dst.b = src[2];
                dst.a = src[3];
            }
            Cached {
                image: Image::from_rgba8(buffer),
                bytes: rgba.len(),
            }
        });
        if self.map.len() >= CACHE_MAX_IMAGES
            || self.bytes + cached.as_ref().map_or(0, |cached| cached.bytes) > CACHE_BUDGET_BYTES
        {
            self.map.clear();
            self.bytes = 0;
        }
        self.bytes += cached.as_ref().map_or(0, |cached| cached.bytes);
        let image = cached.as_ref().map(|cached| cached.image.clone());
        self.map.insert(key, cached);
        image
    }
}

fn file_mtime(path: &str) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Decode a PNG and fit it inside the target box. The stored captures are
/// RGBA8 PNGs; the other cases keep older or hand-copied files readable.
fn decode(path: &str, max_w: u32, max_h: u32) -> Option<(u32, u32, Vec<u8>)> {
    let file = File::open(path).ok()?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0u8; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).ok()?;
    if frame.width == 0 || frame.height == 0 {
        return None;
    }
    let width = frame.width;
    let height = frame.height;
    let pixels = &buffer[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for pixel in pixels.chunks_exact(3) {
                out.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for &gray in pixels {
                out.extend_from_slice(&[gray, gray, gray, 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for pixel in pixels.chunks_exact(2) {
                out.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            out
        }
        png::ColorType::Indexed => return None,
    };
    Some(fit(&rgba, width, height, max_w, max_h))
}

/// Box-filter downscale. Upscaling is left to the renderer so a small capture
/// keeps its pixels crisp; a large screenshot lands at the size its slot can
/// show.
fn fit(src: &[u8], width: u32, height: u32, max_w: u32, max_h: u32) -> (u32, u32, Vec<u8>) {
    let scale = (max_w as f64 / width as f64)
        .min(max_h as f64 / height as f64)
        .min(1.0);
    let target_w = ((width as f64 * scale).round() as u32).max(1);
    let target_h = ((height as f64 * scale).round() as u32).max(1);
    if target_w == width && target_h == height {
        return (width, height, src.to_vec());
    }
    let mut out = vec![0u8; target_w as usize * target_h as usize * 4];
    for ty in 0..target_h {
        let y0 = (u64::from(ty) * u64::from(height) / u64::from(target_h)) as u32;
        let y1 = ((u64::from(ty) + 1) * u64::from(height) / u64::from(target_h))
            .max(u64::from(y0) + 1)
            .min(u64::from(height)) as u32;
        for tx in 0..target_w {
            let x0 = (u64::from(tx) * u64::from(width) / u64::from(target_w)) as u32;
            let x1 = ((u64::from(tx) + 1) * u64::from(width) / u64::from(target_w))
                .max(u64::from(x0) + 1)
                .min(u64::from(width)) as u32;
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * width + x) * 4) as usize;
                    for (channel, value) in sum.iter_mut().enumerate() {
                        *value += u32::from(src[i + channel]);
                    }
                    count += 1;
                }
            }
            let o = ((ty * target_w + tx) * 4) as usize;
            for (channel, value) in sum.iter().enumerate() {
                out[o + channel] = (value / count) as u8;
            }
        }
    }
    (target_w, target_h, out)
}
