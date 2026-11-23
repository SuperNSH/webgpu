//! pixelmatch (mapbox/pixelmatch), as the reference's `checkImage` uses
//! it: YIQ color distance with a threshold, anti-aliased pixels not
//! counted.

fn blend(c: f64, a: f64) -> f64 {
    255.0 + (c - 255.0) * a
}

fn rgb2y(r: f64, g: f64, b: f64) -> f64 {
    r * 0.298_895_31 + g * 0.586_622_47 + b * 0.114_482_23
}

fn rgb2i(r: f64, g: f64, b: f64) -> f64 {
    r * 0.595_977_99 - g * 0.274_176_10 - b * 0.321_801_89
}

fn rgb2q(r: f64, g: f64, b: f64) -> f64 {
    r * 0.211_470_17 - g * 0.522_617_11 + b * 0.311_146_94
}

fn px(img: &[u8], k: usize) -> (f64, f64, f64, f64) {
    let (r, g, b, a) = (img[k], img[k + 1], img[k + 2], img[k + 3]);
    let (r, g, b, a) = (f64::from(r), f64::from(g), f64::from(b), f64::from(a));
    if a < 255.0 {
        let a = a / 255.0;
        (blend(r, a), blend(g, a), blend(b, a), a)
    } else {
        (r, g, b, 1.0)
    }
}

fn color_delta(img1: &[u8], img2: &[u8], k: usize, m: usize, y_only: bool) -> f64 {
    if img1[k..k + 4] == img2[m..m + 4] {
        return 0.0;
    }
    let (r1, g1, b1, _) = px(img1, k);
    let (r2, g2, b2, _) = px(img2, m);
    let y1 = rgb2y(r1, g1, b1);
    let y2 = rgb2y(r2, g2, b2);
    let y = y1 - y2;
    if y_only {
        return y;
    }
    let i = rgb2i(r1, g1, b1) - rgb2i(r2, g2, b2);
    let q = rgb2q(r1, g1, b1) - rgb2q(r2, g2, b2);
    let delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;
    if y1 > y2 { -delta } else { delta }
}

fn has_many_siblings(img: &[u8], x1: usize, y1: usize, w: usize, h: usize) -> bool {
    let (x0, y0) = (x1.saturating_sub(1), y1.saturating_sub(1));
    let (x2, y2) = ((x1 + 1).min(w - 1), (y1 + 1).min(h - 1));
    let pos = (y1 * w + x1) * 4;
    let mut zeroes = usize::from(x1 == x0 || x1 == x2 || y1 == y0 || y1 == y2);
    for x in x0..=x2 {
        for y in y0..=y2 {
            if x == x1 && y == y1 {
                continue;
            }
            let pos2 = (y * w + x) * 4;
            if img[pos..pos + 4] == img[pos2..pos2 + 4] {
                zeroes += 1;
            }
            if zeroes > 2 {
                return true;
            }
        }
    }
    false
}

fn antialiased(img: &[u8], x1: usize, y1: usize, w: usize, h: usize, img2: &[u8]) -> bool {
    let (x0, y0) = (x1.saturating_sub(1), y1.saturating_sub(1));
    let (x2, y2) = ((x1 + 1).min(w - 1), (y1 + 1).min(h - 1));
    let pos = (y1 * w + x1) * 4;
    let mut zeroes = usize::from(x1 == x0 || x1 == x2 || y1 == y0 || y1 == y2);
    let (mut min, mut max) = (0.0, 0.0);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0, 0, 0, 0);
    for x in x0..=x2 {
        for y in y0..=y2 {
            if x == x1 && y == y1 {
                continue;
            }
            let delta = color_delta(img, img, pos, (y * w + x) * 4, true);
            if delta == 0.0 {
                zeroes += 1;
                if zeroes > 2 {
                    return false;
                }
            } else if delta < min {
                min = delta;
                (min_x, min_y) = (x, y);
            } else if delta > max {
                max = delta;
                (max_x, max_y) = (x, y);
            }
        }
    }
    if min == 0.0 || max == 0.0 {
        return false;
    }
    (has_many_siblings(img, min_x, min_y, w, h) && has_many_siblings(img2, min_x, min_y, w, h))
        || (has_many_siblings(img, max_x, max_y, w, h)
            && has_many_siblings(img2, max_x, max_y, w, h))
}

/// How many pixels of RGBA images `a` and `b` differ beyond `threshold`
/// (0 to 1), anti-aliasing not counted.
pub(crate) fn count(a: &[u8], b: &[u8], width: u32, height: u32, threshold: f64) -> usize {
    let (w, h) = (width as usize, height as usize);
    assert_eq!(a.len(), w * h * 4);
    assert_eq!(b.len(), w * h * 4);
    let max_delta = 35215.0 * threshold * threshold;
    let mut diff = 0;
    for y in 0..h {
        for x in 0..w {
            let pos = (y * w + x) * 4;
            let delta = color_delta(a, b, pos, pos, false);
            if delta.abs() > max_delta
                && !(antialiased(a, x, y, w, h, b) || antialiased(b, x, y, w, h, a))
            {
                diff += 1;
            }
        }
    }
    diff
}
