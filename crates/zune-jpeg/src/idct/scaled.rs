/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Reducing reconstructed blocks, for decoding at 1/2, 1/4 or 1/8 of the image's size.
//!
//! Each output sample is the mean of the samples of the block it covers, which is what
//! libjpeg's reduced inverse DCTs compute: the full-size image averaged over 2 by 2, 4 by 4 or
//! 8 by 8 pixels. The block is reconstructed with the decoder's own (vectorized) inverse DCT
//! and averaged; a 1 by 1 output is the block's mean, its DC term, with no inverse DCT at all.
//! Components that are subsampled get larger outputs than full-resolution ones (an 8 by 8
//! chroma block of a 4:2:0 image decoded at half size), so that all of them come out at the
//! same resolution and need no up-sampling.

/// Writes a reduced block from a reconstructed 8 by 8 one into rows `stride` apart.
pub type AverageFn = fn(&[i16; 64], &mut [i16], usize);

/// Write the means of the `8 / W` by `8 / H` groups of samples of `block` (rows of 8) as a
/// `W` by `H` block into `output`, whose rows are `stride` apart. A side of 16 repeats each
/// sample twice along it (a subsampled component whose reduction is less than its subsampling
/// in that direction, as libjpeg's 16-point inverse DCTs do by interpolating).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "a group has at most 64 samples, and their mean is a sample"
)]
pub fn average<const W: usize, const H: usize>(
    block: &[i16; 64], output: &mut [i16], stride: usize,
) {
    // Samples of `block` each output sample covers, across and down.
    let (kw, kh) = ((8 / W).max(1), (8 / H).max(1));
    let count = (kw * kh) as i32;

    for y in 0..H {
        let Some(out) = output.get_mut(y * stride..y * stride + W) else {
            return;
        };
        let row_start = y * 8 / H;
        for (x, sample) in out.iter_mut().enumerate() {
            let column_start = x * 8 / W;
            let mut sum = 0_i32;
            for row in block[row_start * 8..][..kh * 8].chunks_exact(8) {
                for value in &row[column_start..column_start + kw] {
                    sum += i32::from(*value);
                }
            }
            *sample = ((sum + count / 2) / count) as i16;
        }
    }
}

/// The mean of a block, from its dequantized DC coefficient: the 1 by 1 reduction.
#[inline]
pub fn dc_mean(dc: i32) -> i16 {
    (dc.wrapping_add(4 + 1024) >> 3).clamp(0, 255) as i16
}

/// The reduction to `width` by `height` samples per block, both 1, 2, 4, 8 or 16, or `None`
/// for 8 by 8.
pub fn choose_average(width: usize, height: usize) -> Option<AverageFn> {
    macro_rules! sizes {
        ($(($w:literal, $h:literal)),*) => {
            match (width, height) {
                $(($w, $h) => Some(average::<$w, $h> as AverageFn),)*
                _ => None
            }
        };
    }

    sizes!(
        (1, 1), (1, 2), (1, 4), (1, 8), (1, 16),
        (2, 1), (2, 2), (2, 4), (2, 8), (2, 16),
        (4, 1), (4, 2), (4, 4), (4, 8), (4, 16),
        (8, 1), (8, 2), (8, 4), (8, 16),
        (16, 1), (16, 2), (16, 4), (16, 8), (16, 16)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idct::scalar::idct_int;

    /// The full inverse DCT of `input` in floating point, averaged over blocks of `8 / W` by
    /// `8 / H`.
    fn reference(input: &[i32; 64], w: usize, h: usize) -> alloc::vec::Vec<f64> {
        let c = |u: usize| if u == 0 { core::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
        let full = |x: usize, y: usize| -> f64 {
            let mut sum = 0.0;
            for v in 0..8 {
                for u in 0..8 {
                    sum += c(u)
                        * c(v)
                        * f64::from(input[v * 8 + u])
                        * (((2 * x + 1) as f64) * u as f64 * core::f64::consts::PI / 16.0).cos()
                        * (((2 * y + 1) as f64) * v as f64 * core::f64::consts::PI / 16.0).cos();
                }
            }
            (sum / 4.0 + 128.0).clamp(0.0, 255.0)
        };
        let (bw, bh) = (8 / w, 8 / h);
        let mut out = alloc::vec::Vec::new();
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                for yy in 0..bh {
                    for xx in 0..bw {
                        sum += full(x * bw + xx, y * bh + yy);
                    }
                }
                out.push(sum / (bw * bh) as f64);
            }
        }
        out
    }

    #[test]
    fn reduced_blocks_are_the_averaged_full_block() {
        let mut input = [0_i32; 64];
        for (i, value) in input.iter_mut().enumerate() {
            *value = ((i as i32 * 37) % 61) - 30;
        }
        input[0] = 240;
        let mut full = [0_i16; 64];
        idct_int(&mut input.clone(), &mut full, 8);

        for (w, h) in [(4, 4), (2, 2), (4, 8), (8, 4), (2, 4), (1, 2), (2, 1)] {
            let expected = reference(&input, w, h);
            let mut output = [0_i16; 64];
            choose_average(w, h).unwrap()(&full, &mut output, 8);
            for y in 0..h {
                for x in 0..w {
                    let got = f64::from(output[y * 8 + x]);
                    let want = expected[y * w + x];
                    assert!(
                        (got - want).abs() <= 1.0,
                        "{w}x{h} ({x}, {y}): {got} against {want}"
                    );
                }
            }
        }

        let mean = reference(&input, 1, 1)[0];
        assert!((f64::from(dc_mean(input[0])) - mean).abs() <= 1.0);
    }
}
