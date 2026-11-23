use agent_desktop_core::Rect;

use super::*;

fn font() -> LabelFont {
    LabelFont::load().expect("a sans-serif font: install fontconfig and fonts-dejavu-core")
}

#[test]
fn text_is_measured_wider_for_more_characters_and_a_larger_em() {
    let font = font();
    let short = font.measure("click", 12.5).expect("width");
    let long = font.measure("click the button", 12.5).expect("width");
    let large = font.measure("click", 25.0).expect("width");
    assert!(short > 10.0 && short < 60.0, "{short}");
    assert!(long > short);
    assert!((large / short - 2.0).abs() < 0.05, "{large} {short}");
}

#[test]
fn text_that_does_not_fit_ends_in_an_ellipsis() {
    let font = font();
    assert_eq!(font.fitted("Save", 12.5, 200.0).as_deref(), Some("Save"));
    let cut = font
        .fitted("Save the document under a new name", 12.5, 60.0)
        .expect("fits with an ellipsis");
    assert!(cut.ends_with('\u{2026}'), "{cut}");
    assert!(font.measure(&cut, 12.5).expect("width") <= 60.5);
    assert_eq!(font.fitted("Save", 12.5, 0.0), None);
}

#[test]
fn drawn_text_is_white_and_stays_inside_its_frame() {
    let font = font();
    let mut surface = Surface::transparent(120, 30).expect("surface");
    let frame = Rect {
        x: 10.0,
        y: 5.0,
        width: 100.0,
        height: 20.0,
    };
    font.draw(&mut surface, "Label", &frame, 12.5);
    let mut inked = 0;
    for y in 0..30 {
        for x in 0..120 {
            let pixel = surface.pixel_at(x, y).expect("pixel");
            if pixel >> 24 == 0 {
                continue;
            }
            inked += 1;
            assert!(
                (10..110).contains(&x) && (5..25).contains(&y),
                "ink at {x},{y}"
            );
            let alpha = pixel >> 24;
            assert_eq!(pixel & 0xff, alpha, "white premultiplied");
        }
    }
    assert!(inked > 20, "{inked}");
}

#[test]
fn a_file_that_is_not_a_font_is_refused() {
    let path =
        std::env::temp_dir().join(format!("agent-desktop-not-a-font-{}", std::process::id()));
    std::fs::write(&path, b"not a font").expect("written");
    assert!(read_font(&path).is_none());
    let _ = std::fs::remove_file(&path);
}
