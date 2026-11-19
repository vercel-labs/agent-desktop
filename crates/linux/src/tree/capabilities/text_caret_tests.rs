use std::collections::BTreeMap;

use super::*;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

#[derive(Default)]
struct FakeText {
    caret: Option<i32>,
    screen: Option<BTreeMap<i32, Extents>>,
    window: Option<BTreeMap<i32, Extents>>,
    breaks: Vec<i32>,
}

impl FakeText {
    fn at(caret: i32) -> Self {
        Self {
            caret: Some(caret),
            ..Self::default()
        }
    }

    fn screen(mut self, characters: &[(i32, Extents)]) -> Self {
        self.screen = Some(characters.iter().copied().collect());
        self
    }

    fn window(mut self, characters: &[(i32, Extents)]) -> Self {
        self.window = Some(characters.iter().copied().collect());
        self
    }
}

impl CaretSource for FakeText {
    fn caret_offset(&mut self) -> Result<Option<i32>, AdapterError> {
        Ok(self.caret)
    }

    fn character_extents(
        &mut self,
        offset: i32,
        space: CoordSpace,
    ) -> Result<Option<Extents>, AdapterError> {
        let characters = match space {
            CoordSpace::Screen => &self.screen,
            CoordSpace::Window => &self.window,
        };
        Ok(characters
            .as_ref()
            .map(|characters| characters.get(&offset).copied().unwrap_or(rect(0, 0, 0, 0))))
    }

    fn ends_line(&mut self, offset: i32) -> Result<bool, AdapterError> {
        Ok(self.breaks.contains(&offset))
    }
}

#[test]
fn the_character_at_the_caret_marks_it_including_a_zero_width_end() {
    let mut gtk3 =
        FakeText::at(11).screen(&[(10, rect(144, 106, 9, 21)), (11, rect(153, 106, 0, 21))]);
    assert_eq!(
        locate_caret(&mut gtk3).expect("read"),
        Some((rect(153, 106, 2, 21), CoordSpace::Screen))
    );
}

#[test]
fn an_end_with_no_character_takes_the_right_edge_of_the_one_before() {
    let mut chromium = FakeText::at(11).screen(&[(10, rect(131, 164, 8, 17))]);
    assert_eq!(
        locate_caret(&mut chromium).expect("read"),
        Some((rect(139, 164, 2, 17), CoordSpace::Screen))
    );
}

#[test]
fn a_caret_after_a_line_break_opens_the_next_line() {
    let mut text =
        FakeText::at(18).screen(&[(0, rect(62, 223, 8, 15)), (17, rect(115, 238, 1, 15))]);
    text.breaks = vec![17];
    assert_eq!(
        locate_caret(&mut text).expect("read"),
        Some((rect(62, 253, 2, 15), CoordSpace::Screen))
    );
}

#[test]
fn a_toolkit_without_screen_coordinates_is_read_in_window_coordinates() {
    let mut gtk4 = FakeText::at(5).window(&[(5, rect(69, 125, 9, 20))]);
    assert_eq!(
        locate_caret(&mut gtk4).expect("read"),
        Some((rect(69, 125, 2, 20), CoordSpace::Window))
    );
}

#[test]
fn a_node_that_places_no_character_has_no_caret() {
    assert_eq!(locate_caret(&mut FakeText::at(5)).expect("read"), None);
}

#[test]
fn an_unfocused_field_with_no_caret_offset_has_no_caret() {
    let mut text = FakeText::at(-1).screen(&[(0, rect(63, 164, 9, 17))]);
    assert_eq!(locate_caret(&mut text).expect("read"), None);
    let mut unread = FakeText::default().screen(&[(0, rect(63, 164, 9, 17))]);
    assert_eq!(locate_caret(&mut unread).expect("read"), None);
}

#[test]
fn an_empty_field_with_nothing_placed_has_no_caret() {
    let mut empty = FakeText::at(0).screen(&[]);
    assert_eq!(locate_caret(&mut empty).expect("read"), None);
}

#[test]
fn a_short_line_still_gets_a_visible_caret() {
    let mut text = FakeText::at(0).screen(&[(0, rect(10, 10, 4, 3))]);
    assert_eq!(
        locate_caret(&mut text).expect("read"),
        Some((rect(10, 10, 2, MIN_CARET_HEIGHT), CoordSpace::Screen))
    );
}
