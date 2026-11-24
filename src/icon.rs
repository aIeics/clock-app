//! The window/taskbar icon, drawn in code so no image file or decoding crate is
//! needed. Shapes are sampled several times per pixel for smooth edges.

use eframe::egui;

const SIZE: u32 = 64;
/// Samples per pixel, per axis. 4 means 16 samples per pixel.
const SAMPLES: u32 = 4;

const BLUE: [f32; 4] = [0.231, 0.510, 0.965, 1.0]; // #3B82F6
const FACE: [f32; 4] = [0.980, 0.984, 0.992, 1.0]; // #FAFBFD
const INK: [f32; 4] = [0.118, 0.161, 0.231, 1.0]; // #1E293B
const TICK: [f32; 4] = [0.580, 0.639, 0.722, 1.0]; // #94A3B8

pub fn app_icon() -> egui::IconData {
    let size = SIZE as f32;
    let center = size / 2.0;
    let samples = (SAMPLES * SAMPLES) as f32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut pixel = [0.0_f32; 4];

            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;

                    // Layers, back to front, composited for this one sample.
                    let mut sample = [0.0_f32; 4];
                    if rounded_square(px, py, center, 14.0) {
                        sample = over(sample, BLUE);
                    }
                    if circle(px, py, center, center, 22.0) {
                        sample = over(sample, FACE);
                    }
                    if tick(px, py, center) {
                        sample = over(sample, TICK);
                    }
                    if hand(px, py, center) {
                        sample = over(sample, INK);
                    }

                    // Add it premultiplied, so covered samples add up instead of
                    // overwriting each other.
                    for channel in 0..3 {
                        pixel[channel] += sample[channel] * sample[3];
                    }
                    pixel[3] += sample[3];
                }
            }

            // `pixel` holds the sum of the covered samples: divide the colour by
            // the covered count (straight alpha) and the alpha by all samples.
            let covered = pixel[3];
            if covered == 0.0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            for channel in &pixel[..3] {
                rgba.push(to_u8(channel / covered));
            }
            rgba.push(to_u8(covered / samples));
        }
    }

    egui::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

fn rounded_square(x: f32, y: f32, half: f32, radius: f32) -> bool {
    let dx = (x - half).abs() - (half - radius);
    let dy = (y - half).abs() - (half - radius);
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    outside + dx.max(dy).min(0.0) - radius <= 0.0
}

fn circle(x: f32, y: f32, cx: f32, cy: f32, radius: f32) -> bool {
    (x - cx).powi(2) + (y - cy).powi(2) <= radius * radius
}

/// The four marks at 12, 3, 6 and 9 o'clock.
fn tick(x: f32, y: f32, center: f32) -> bool {
    let radius = 17.5;
    [(0.0, -radius), (radius, 0.0), (0.0, radius), (-radius, 0.0)]
        .iter()
        .any(|(dx, dy)| circle(x, y, center + dx, center + dy, 1.4))
}

/// The hands (10:10) and the pin in the middle.
fn hand(x: f32, y: f32, center: f32) -> bool {
    let hour_end = (center - 7.8, center - 4.5);
    let minute_end = (center + 7.0, center - 12.1);

    segment(x, y, center, center, hour_end.0, hour_end.1, 2.0)
        || segment(x, y, center, center, minute_end.0, minute_end.1, 1.5)
        || circle(x, y, center, center, 2.6)
}

fn segment(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32, half_width: f32) -> bool {
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (x - ax, y - ay);
    let length_squared = abx * abx + aby * aby;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        ((apx * abx + apy * aby) / length_squared).clamp(0.0, 1.0)
    };
    let (cx, cy) = (ax + abx * t, ay + aby * t);

    (x - cx).powi(2) + (y - cy).powi(2) <= half_width * half_width
}

/// Puts `src` over `dst` (both straight alpha, not premultiplied).
fn over(dst: [f32; 4], src: [f32; 4]) -> [f32; 4] {
    let out_alpha = src[3] + dst[3] * (1.0 - src[3]);
    if out_alpha == 0.0 {
        return [0.0; 4];
    }

    let mut out = [0.0; 4];
    for channel in 0..3 {
        out[channel] = (src[channel] * src[3] + dst[channel] * dst[3] * (1.0 - src[3])) / out_alpha;
    }
    out[3] = out_alpha;
    out
}

fn to_u8(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(icon: &egui::IconData, x: u32, y: u32) -> [u8; 4] {
        let start = ((y * icon.width + x) * 4) as usize;
        icon.rgba[start..start + 4].try_into().unwrap()
    }

    #[test]
    fn has_one_rgba_value_per_pixel() {
        let icon = app_icon();
        assert_eq!(icon.width, SIZE);
        assert_eq!(icon.height, SIZE);
        assert_eq!(icon.rgba.len(), (SIZE * SIZE * 4) as usize);
    }

    #[test]
    fn corners_are_transparent_and_the_middle_is_opaque() {
        let icon = app_icon();
        assert_eq!(pixel(&icon, 0, 0)[3], 0);
        assert_eq!(pixel(&icon, SIZE - 1, SIZE - 1)[3], 0);
        assert_eq!(pixel(&icon, SIZE / 2, SIZE / 2)[3], 255);
    }

    #[test]
    fn edges_are_blended() {
        let icon = app_icon();
        let partial = (0..SIZE).any(|y| {
            (0..SIZE).any(|x| {
                let alpha = pixel(&icon, x, y)[3];
                alpha > 0 && alpha < 255
            })
        });
        assert!(partial, "the rounded corners should be antialiased");
    }
}
