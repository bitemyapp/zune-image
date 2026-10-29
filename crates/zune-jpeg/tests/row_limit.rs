/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Decoding only an image's first rows (`JpegDecoder::set_row_limit`).

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

fn path(name: &str) -> String {
    format!("{}/../../test-images/jpeg/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The decoded size and pixels, with the first `rows` rows only if `rows` is given, and
/// whether the decoder cut the image short.
fn decode(data: &[u8], scale: u8, rows: Option<usize>) -> (usize, usize, Vec<u8>, bool) {
    let options = DecoderOptions::default().jpeg_set_scale_denominator(scale);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    let cut = rows.is_some_and(|rows| decoder.set_row_limit(rows));
    let (width, height) = decoder.output_dimensions().unwrap();
    let pixels = decoder.decode().unwrap();
    assert_eq!(
        pixels.len(),
        width * height * decoder.output_colorspace().unwrap().num_components()
    );
    (width, height, pixels, cut)
}

/// Cut decodes give the whole image's rows, but for a few above the cut where chroma is
/// upsampled from rows that weren't decoded.
#[test]
fn first_rows_match_the_whole_image() {
    for name in [
        "medium_no_samp_2500x1786.jpg",
        "medium_horiz_samp_2500x1786.jpg",
        "medium_vertical_samp_2500x1786.jpg",
        "fox410.jpg"
    ] {
        let data = std::fs::read(path(name)).unwrap();
        for scale in [1, 2, 8] {
            let (width, full_height, full, _) = decode(&data, scale, None);
            let image_height = decode(&data, 1, None).1;
            for rows in [1, 7, 16, 17, 500, 1000]
                .into_iter()
                .filter(|&r| r < image_height)
            {
                let (cut_width, height, pixels, cut) = decode(&data, scale, Some(rows));
                assert!(cut, "{name} not cut");
                assert_eq!(cut_width, width);
                assert_eq!(height, rows.div_ceil(usize::from(scale)), "{name} {rows}");
                assert!(height <= full_height);

                // Rows more than 16 output rows above the cut are exact.
                let exact = height.saturating_sub(16);
                let stride = pixels.len() / height;
                assert_eq!(
                    pixels[..exact * stride],
                    full[..exact * stride],
                    "{name} at 1/{scale}, {rows} rows"
                );
            }
        }
    }
}

/// Progressive images, and limits past the image's height, decode the whole image.
#[test]
fn others_decode_whole() {
    let data = std::fs::read(path("progressive_restart_420.jpg")).unwrap();
    let (_, full_height, full, _) = decode(&data, 1, None);
    let (_, height, pixels, cut) = decode(&data, 1, Some(full_height / 2));
    assert!(!cut);
    assert_eq!((height, pixels), (full_height, full));

    let data = std::fs::read(path("fox410.jpg")).unwrap();
    let (_, full_height, full, _) = decode(&data, 1, None);
    let (_, height, pixels, cut) = decode(&data, 1, Some(full_height + 5));
    assert!(!cut);
    assert_eq!((height, pixels), (full_height, full));
}
