use super::*;
use crate::notifications::fake::{FakeWorld, lease, log, reading, spec};
use crate::notifications::list::read_listing;
use agent_desktop_core::InteractionPolicy;

fn first_entry_and_list(world: &FakeWorld) -> (Entry, Vec<Entry>) {
    let before = read_listing(world, lease(600).deadline()).expect("listing");
    (before[0].clone(), before)
}

fn close(world: &FakeWorld, entry: &Entry, before: &[Entry]) -> Result<Vec<Entry>, Unclosed> {
    close_entry(
        world,
        entry,
        before,
        InteractionPolicy::headed(),
        &lease(600),
    )
}

#[test]
fn an_entry_with_no_header_button_is_not_pressed_at_all() {
    let world = FakeWorld::new(&log(), vec![reading(&[spec("Mail", "a")])]);
    let (mut entry, before) = first_entry_and_list(&world);
    entry.header.clear();
    let unclosed = close(&world, &entry, &before).expect_err("no control means no press");
    assert!(!unclosed.unread);
    assert!(world.clicks().is_empty());
}

#[test]
fn a_bystander_leaving_with_the_target_is_not_a_clean_close() {
    let world = FakeWorld::new(
        &log(),
        vec![
            reading(&[spec("Mail", "a"), spec("Chat", "b")]),
            reading(&[]),
        ],
    );
    let (entry, before) = first_entry_and_list(&world);
    let unclosed = close(&world, &entry, &before).expect_err("a bystander left");
    assert!(unclosed.error.message.contains("other than the target"));
}

#[test]
fn an_entry_that_stays_after_every_header_button_is_a_failure() {
    let world = FakeWorld::new(&log(), vec![reading(&[spec("Mail", "a")])]);
    let (entry, before) = first_entry_and_list(&world);
    assert!(close(&world, &entry, &before).is_err());
    assert_eq!(world.clicks(), vec!["/Mail:a/close", "/Mail:a/expand"]);
}

#[test]
fn a_failure_after_a_sent_press_keeps_the_presss_delivery() {
    let log = log();
    let mut world = FakeWorld::new(
        &log,
        vec![reading(&[spec("Mail", "a")]), reading(&[spec("Mail", "a")])],
    );
    world.faults.click_refused_from = Some(1);
    let (entry, before) = first_entry_and_list(&world);
    let unclosed = close(&world, &entry, &before).expect_err("the second press was refused");
    assert_eq!(world.clicks(), vec!["/Mail:a/close", "/Mail:a/expand"]);
    assert_eq!(
        unclosed.error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
}
