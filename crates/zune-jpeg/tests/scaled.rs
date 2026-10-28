/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Decoding at 1/2, 1/4 and 1/8 of the image's size
//! (`DecoderOptions::jpeg_set_scale_denominator`).

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

fn decode(data: &[u8], (across, down): (u8, u8)) -> (usize, usize, usize, Vec<u8>) {
    let options = DecoderOptions::default().jpeg_set_scale_denominators(across, down);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    let (width, height) = decoder.output_dimensions().unwrap();
    let pixels = decoder.decode().unwrap();
    let channels = decoder.output_colorspace().unwrap().num_components();
    assert_eq!(pixels.len(), width * height * channels, "output size");
    (width, height, channels, pixels)
}

/// The mean absolute difference between a reduced decode and the full decode averaged over
/// blocks of `across` by `down` pixels (fewer at the right and bottom edges).
fn difference_from_averaged_full(data: &[u8], (across, down): (u8, u8)) -> f64 {
    let (full_width, full_height, channels, full) = decode(data, (1, 1));
    let (width, height, reduced_channels, reduced) = decode(data, (across, down));
    let (across, down) = (usize::from(across), usize::from(down));
    assert_eq!(channels, reduced_channels);
    assert_eq!(
        (width, height),
        (full_width.div_ceil(across), full_height.div_ceil(down))
    );

    let mut sum = 0_u64;
    for y in 0..height {
        for x in 0..width {
            for channel in 0..channels {
                let (mut total, mut count) = (0_u32, 0_u32);
                for yy in y * down..((y + 1) * down).min(full_height) {
                    for xx in x * across..((x + 1) * across).min(full_width) {
                        total += u32::from(full[(yy * full_width + xx) * channels + channel]);
                        count += 1;
                    }
                }
                let expected = (total + count / 2) / count;
                let got = u32::from(reduced[(y * width + x) * channels + channel]);
                sum += u64::from(expected.abs_diff(got));
            }
        }
    }

    sum as f64 / (width * height * channels) as f64
}

#[test]
fn reduced_decodes_are_the_averaged_full_decode() {
    // Baseline and progressive, 4:4:4, 4:2:0, unusual sampling factors, one and four
    // components, a multi-scan baseline image.
    let images: [(&str, &[u8]); 8] = [
        ("2029", include_bytes!("../../../test-images/jpeg/2029.jpg")),
        (
            "progressive_dc_huffman_table_1",
            include_bytes!("../../../test-images/jpeg/progressive_dc_huffman_table_1.jpg")
        ),
        (
            "progressive_restart_420",
            include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg")
        ),
        ("sampling_factors", include_bytes!("../../../test-images/jpeg/sampling_factors.jpg")),
        (
            "weid_sampling_factors",
            include_bytes!("../../../test-images/jpeg/weid_sampling_factors.jpg")
        ),
        (
            "down_sampled_grayscale_prog",
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg")
        ),
        ("cymk", include_bytes!("../../../test-images/jpeg/cymk.jpg")),
        ("sos_news", include_bytes!("../../../test-images/jpeg/sos_news.jpeg"))
    ];

    for (name, data) in images {
        for scale in [(2, 2), (4, 4), (8, 8), (8, 1), (1, 4), (2, 8)] {
            let difference = difference_from_averaged_full(data, scale);
            // Chroma is reconstructed from the coefficients rather than up-sampled, and
            // samples are clamped before averaging, so a few levels apart on average.
            assert!(difference < 2.5, "{name} at {scale:?}: {difference}");
        }
    }
}

#[test]
fn unsupported_reductions_decode_at_full_size() {
    // Luma 4 by 2, chroma 1 by 1: unreduced across, chroma would need blocks 32 samples wide.
    let data = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let options = DecoderOptions::default().jpeg_set_scale_denominators(1, 2);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.output_dimensions(), decoder.dimensions());
    let pixels = decoder.decode().unwrap();
    let (width, height) = decoder.dimensions().unwrap();
    assert_eq!(pixels.len(), width * height * 3);

    // Halved, chroma blocks are 16 by 8 samples.
    let (width, height, _, _) = decode(data, (2, 2));
    assert_eq!((width, height), (303, 403));
    assert!(difference_from_averaged_full(data, (2, 2)) < 2.5);

    // Across only, they're 8 by 16.
    let (width, height, _, _) = decode(data, (4, 1));
    assert_eq!((width, height), (152, 806));
    assert!(difference_from_averaged_full(data, (4, 1)) < 2.5);
}

#[test]
fn denominators_round_down_to_a_power_of_two() {
    for (requested, applied) in [(0, 1), (1, 1), (3, 2), (5, 4), (7, 4), (9, 8), (255, 8)] {
        assert_eq!(
            DecoderOptions::default()
                .jpeg_set_scale_denominator(requested)
                .jpeg_get_scale_denominators(),
            (applied, applied)
        );
    }
}
