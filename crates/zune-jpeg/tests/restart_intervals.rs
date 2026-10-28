/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Restart intervals that don't divide the MCUs in a row.

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

fn decode(data: &[u8], colorspace: ColorSpace) -> Vec<u8> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    JpegDecoder::new_with_options(ZCursor::new(data), options).decode().unwrap()
}

/// A grayscale image 13 MCUs wide with a restart every 4 MCUs, so that entropy segments end
/// in the row after the one they start in, decodes as the same image without restarts.
///
/// Both were encoded by libjpeg-turbo's `cjpeg -grayscale -quality 85`, one with
/// `-restart 4B`, from a crop of `medium_no_samp_2500x1786.jpg`, so they hold the same
/// coefficients.
#[test]
fn restart_segments_across_rows() {
    let with_restarts = include_bytes!("../../../test-images/jpeg/restart_interval_across_rows.jpg");
    let without = include_bytes!("../../../test-images/jpeg/restart_interval_across_rows_reference.jpg");

    for colorspace in [ColorSpace::Luma, ColorSpace::RGB] {
        assert!(
            decode(with_restarts, colorspace) == decode(without, colorspace),
            "{colorspace:?} output differs"
        );
    }
}
