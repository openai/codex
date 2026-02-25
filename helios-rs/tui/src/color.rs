pub(crate) fn is_light(bg: (u8, u8, u8)) -> bool {
    let (r, g, b) = bg;
    let y = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    y > 128.0
}

pub(crate) fn blend(fg: (u8, u8, u8), bg: (u8, u8, u8), alpha: f32) -> (u8, u8, u8) {
    let r = (fg.0 as f32 * alpha + bg.0 as f32 * (1.0 - alpha)) as u8;
    let g = (fg.1 as f32 * alpha + bg.1 as f32 * (1.0 - alpha)) as u8;
    let b = (fg.2 as f32 * alpha + bg.2 as f32 * (1.0 - alpha)) as u8;
    (r, g, b)
}

/// Returns the perceptual color distance between two RGB colors.
/// Uses the CIE76 formula (Euclidean distance in Lab space approximation).
pub(crate) fn perceptual_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    // Convert sRGB to linear RGB
    fn srgb_to_linear(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    // Convert RGB to XYZ
    fn rgb_to_xyz(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
        let r = srgb_to_linear(r);
        let g = srgb_to_linear(g);
        let b = srgb_to_linear(b);

        let x = r * 0.4124 + g * 0.3576 + b * 0.1805;
        let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
        let z = r * 0.0193 + g * 0.1192 + b * 0.9505;
        (x, y, z)
    }

    // Convert XYZ to Lab
    fn xyz_to_lab(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        // D65 reference white
        let xr = x / 0.95047;
        let yr = y / 1.00000;
        let zr = z / 1.08883;

        fn f(t: f32) -> f32 {
            if t > 0.008856 {
                t.powf(1.0 / 3.0)
            } else {
                7.787 * t + 16.0 / 116.0
            }
        }

        let fx = f(xr);
        let fy = f(yr);
        let fz = f(zr);

        let l = 116.0 * fy - 16.0;
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);
        (l, a, b)
    }

    let (x1, y1, z1) = rgb_to_xyz(a.0, a.1, a.2);
    let (x2, y2, z2) = rgb_to_xyz(b.0, b.1, b.2);

    let (l1, a1, b1) = xyz_to_lab(x1, y1, z1);
    let (l2, a2, b2) = xyz_to_lab(x2, y2, z2);

    let dl = l1 - l2;
    let da = a1 - a2;
    let db = b1 - b2;

    (dl * dl + da * da + db * db).sqrt()
}
