use std::process::Command;

pub(crate) fn xwd_pixel(window: u32, x: usize, y: usize, dir: &str) -> [u8; 3] {
    let path = format!("{dir}/flat.xwd");
    let output = Command::new("xwd")
        .args(["-silent", "-id", &window.to_string(), "-out", &path])
        .output()
        .expect("xwd runs");
    assert!(
        output.status.success(),
        "xwd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let file = std::fs::read(&path).expect("xwd output");
    let field = |index: usize| {
        u32::from_be_bytes(
            file[index * 4..index * 4 + 4]
                .try_into()
                .expect("four bytes"),
        )
    };
    let (header, colours) = (field(0) as usize, field(19) as usize);
    let (big_endian, bits, line) = (field(7) == 1, field(11) as usize, field(12) as usize);
    let bytes = bits / 8;
    let start = header + colours * 12 + y * line + x * bytes;
    let pixel = &file[start..start + bytes];
    let value = if big_endian {
        pixel.iter().fold(0u32, |v, b| (v << 8) | u32::from(*b))
    } else {
        pixel
            .iter()
            .rev()
            .fold(0u32, |v, b| (v << 8) | u32::from(*b))
    };
    let channel = |mask: u32| ((value & mask) >> mask.trailing_zeros()) as u8;
    [channel(field(14)), channel(field(15)), channel(field(16))]
}
