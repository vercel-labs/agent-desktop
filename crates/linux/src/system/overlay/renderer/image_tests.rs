use super::*;

fn png(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(data).expect("data");
    writer.finish().expect("finish");
    bytes
}

#[test]
fn an_rgba_png_decodes_to_premultiplied_pixels() {
    let bytes = png(
        2,
        1,
        png::ColorType::Rgba,
        &[255, 0, 0, 255, 0, 0, 255, 128],
    );
    let picture = decode(&bytes).expect("decodes");
    assert_eq!((picture.width, picture.height), (2, 1));
    assert_eq!(picture.pixels, vec![0xffff_0000, 0x8000_0080]);
}

#[test]
fn an_rgb_png_is_opaque() {
    let bytes = png(1, 1, png::ColorType::Rgb, &[10, 20, 30]);
    assert_eq!(decode(&bytes).expect("decodes").pixels, vec![0xff0a_141e]);
}

#[test]
fn bytes_that_are_not_a_png_or_exceed_the_side_limit_are_refused() {
    assert!(decode(b"GIF89a").is_none());
    let wide = png(1025, 1, png::ColorType::Rgb, &[0; 1025 * 3]);
    assert!(decode(&wide).is_none());
}

#[test]
fn a_missing_file_is_refused() {
    assert!(load("/nonexistent/agent-desktop-cursor.png").is_none());
}

#[test]
fn scaling_samples_the_nearest_source_pixel() {
    let picture = CursorPicture {
        width: 2,
        height: 1,
        pixels: vec![0xff00_00ff, 0xffff_0000],
    };
    let doubled = scaled(&picture, 2.0).expect("scaled");
    assert_eq!((doubled.width, doubled.height), (4, 2));
    assert_eq!(
        doubled.pixels[..4],
        [0xff00_00ff, 0xff00_00ff, 0xffff_0000, 0xffff_0000]
    );
    let halved = scaled(&picture, 0.5).expect("scaled");
    assert_eq!((halved.width, halved.height), (1, 1));
}
