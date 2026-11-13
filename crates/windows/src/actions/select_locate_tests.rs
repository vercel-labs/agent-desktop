use super::*;
use agent_desktop_core::ErrorCode;

struct ScrollCapabilities {
    scrollable: bool,
    reports_position: bool,
    can_return: bool,
}

struct Fake {
    windows: Vec<Vec<u8>>,
    named: Vec<u8>,
    at: usize,
    scroll: ScrollCapabilities,
    lookup: Option<u8>,
    log: Vec<String>,
}

impl Fake {
    fn new(windows: Vec<Vec<u8>>, named: Vec<u8>) -> Self {
        Self {
            windows,
            named,
            at: 0,
            scroll: ScrollCapabilities {
                scrollable: true,
                reports_position: true,
                can_return: true,
            },
            lookup: None,
            log: Vec::new(),
        }
    }
}

impl ItemSource for Fake {
    type Item = u8;

    fn scan(&mut self, prior: Option<&Self::Item>) -> Result<Option<Self::Item>, AdapterError> {
        self.log.push("scan".to_string());
        let collected: Vec<u8> = self.windows[self.at]
            .iter()
            .copied()
            .filter(|id| self.named.contains(id))
            .collect();
        let Some(&first) = collected.first() else {
            return Ok(None);
        };
        if collected.iter().any(|&id| id != first) {
            return Err(AdapterError::ambiguous_target("two items"));
        }
        if let Some(&expected) = prior
            && collected.iter().any(|&id| id != expected)
        {
            return Err(AdapterError::ambiguous_target("two items"));
        }
        Ok(Some(first))
    }

    fn hides_items_behind_a_scroll(&self) -> bool {
        self.scroll.scrollable
    }

    fn position(&self) -> Option<f64> {
        self.scroll.reports_position.then_some(self.at as f64)
    }

    fn scroll_step(&mut self) -> Result<RealizeScrollStep, AdapterError> {
        self.log.push("step".to_string());
        if !self.scroll.scrollable {
            return Ok(RealizeScrollStep::Stop);
        }
        if self.at + 1 >= self.windows.len() {
            return Ok(RealizeScrollStep::Window { end_of_range: true });
        }
        self.at += 1;
        Ok(RealizeScrollStep::Window {
            end_of_range: false,
        })
    }

    fn scroll_to(&mut self, position: f64) -> Result<bool, AdapterError> {
        self.log.push(format!("scroll_to({position})"));
        if !self.scroll.can_return {
            return Ok(false);
        }
        self.at = position as usize;
        Ok(true)
    }

    fn realize(&mut self) -> Result<Option<Self::Item>, AdapterError> {
        self.log.push("realize".to_string());
        Ok(self.lookup)
    }
}

fn found(located: Located<u8>) -> Option<(u8, bool)> {
    match located {
        Located::Found { item, realized } => Some((item, realized)),
        Located::Absent | Located::ScrollDenied => None,
    }
}

#[test]
fn a_headless_search_that_finds_the_item_never_scrolls_or_asks_the_container() {
    let mut fake = Fake::new(vec![vec![1]], vec![1]);
    let located = locate(&mut fake, false).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert!(!fake.log.iter().any(|entry| entry == "step"));
    assert!(!fake.log.iter().any(|entry| entry == "realize"));
}

#[test]
fn a_headless_miss_in_a_scrollable_container_is_a_denied_scroll() {
    let mut fake = Fake::new(vec![vec![9]], vec![1]);
    let located = locate(&mut fake, false).unwrap();
    assert!(matches!(located, Located::ScrollDenied));
    assert_eq!(fake.log, vec!["scan"]);
}

#[test]
fn a_headless_miss_in_a_container_that_cannot_scroll_is_absent() {
    let mut fake = Fake::new(vec![vec![9]], vec![1]);
    fake.scroll.scrollable = false;
    let located = locate(&mut fake, false).unwrap();
    assert!(matches!(located, Located::Absent));
}

#[test]
fn a_miss_asks_the_container_before_any_scroll_and_stops_when_it_answers() {
    let mut fake = Fake::new(vec![vec![9]], vec![1]);
    fake.lookup = Some(7);
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((7, true)));
    assert_eq!(fake.log, vec!["scan", "realize"]);
}

#[test]
fn the_container_is_not_asked_when_the_walk_already_found_the_item() {
    let mut fake = Fake::new(vec![vec![1]], vec![1]);
    fake.scroll.scrollable = false;
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert!(!fake.log.iter().any(|entry| entry == "realize"));
    assert_eq!(
        fake.log
            .iter()
            .filter(|entry| entry.as_str() == "scan")
            .count(),
        1
    );
}

#[test]
fn every_scroll_window_is_scanned() {
    let mut fake = Fake::new(vec![vec![9], vec![8], vec![1]], vec![1]);
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert_eq!(
        fake.log,
        vec![
            "scan", "realize", "step", "scan", "step", "scan", "step", "scan", "scan"
        ]
    );
}

#[test]
fn a_failed_scroll_ends_the_sweep_without_another_scan() {
    let mut fake = Fake::new(vec![vec![1]], vec![1]);
    fake.scroll.scrollable = false;
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert_eq!(fake.log, vec!["scan", "step"]);
}

#[test]
fn a_second_item_in_a_later_window_is_ambiguous() {
    let mut fake = Fake::new(vec![vec![1], vec![2]], vec![1, 2]);
    let err = locate(&mut fake, true).err().unwrap();
    assert_eq!(err.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn a_row_found_first_and_scrolled_out_of_the_list_is_found_again_where_it_was_seen() {
    let mut fake = Fake::new(vec![vec![1], vec![2], vec![3]], vec![1]);
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert!(fake.log.iter().any(|entry| entry == "scroll_to(0)"));
    assert_eq!(fake.at, 0);
}

#[test]
fn a_row_built_in_the_middle_of_the_sweep_is_found_again_where_it_was_seen() {
    let mut fake = Fake::new(vec![vec![9], vec![1], vec![3]], vec![1]);
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert!(fake.log.iter().any(|entry| entry == "scroll_to(1)"));
}

#[test]
fn a_row_still_in_the_list_after_the_sweep_is_used_without_scrolling_back() {
    let mut fake = Fake::new(vec![vec![1], vec![1, 2]], vec![1]);
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, false)));
    assert!(!fake.log.iter().any(|entry| entry.starts_with("scroll_to")));
}

#[test]
fn the_containers_own_lookup_is_tried_before_scrolling_back() {
    let mut fake = Fake::new(vec![vec![1], vec![2]], vec![1]);
    fake.lookup = Some(1);
    let located = locate(&mut fake, true).unwrap();
    assert_eq!(found(located), Some((1, true)));
    assert!(!fake.log.iter().any(|entry| entry.starts_with("scroll_to")));
}

#[test]
fn a_lost_row_in_a_container_that_cannot_scroll_back_is_absent() {
    let mut fake = Fake::new(vec![vec![1], vec![2], vec![3]], vec![1]);
    fake.scroll.can_return = false;
    let located = locate(&mut fake, true).unwrap();
    assert!(matches!(located, Located::Absent));
}

#[test]
fn a_lost_row_without_a_recorded_position_is_absent() {
    let mut fake = Fake::new(vec![vec![1], vec![2], vec![3]], vec![1]);
    fake.scroll.reports_position = false;
    let located = locate(&mut fake, true).unwrap();
    assert!(matches!(located, Located::Absent));
    assert!(!fake.log.iter().any(|entry| entry.starts_with("scroll_to")));
}

#[test]
fn the_sweep_stops_after_its_scroll_budget() {
    let mut fake = Fake::new(vec![Vec::new(); 40], vec![1]);
    let located = locate(&mut fake, true).unwrap();
    assert!(matches!(located, Located::Absent));
    assert_eq!(
        fake.log
            .iter()
            .filter(|entry| entry.as_str() == "step")
            .count(),
        MAX_REALIZE_SCROLLS as usize
    );
}
