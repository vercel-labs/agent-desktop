use super::*;
use crate::{AdapterError, ErrorCode};

fn target_lost() -> AdapterError {
    AdapterError::new(ErrorCode::StaleRef, "the target lost focus")
        .with_details(json!({ "physical_delivery_started": false }))
}

#[test]
fn a_stop_after_delivered_chunks_says_physical_delivery_started() {
    let details = target_lost()
        .with_details(text_chunk_progress(2, 5))
        .details
        .unwrap();
    assert_eq!(details["physical_delivery_started"], true);
    assert_eq!(details["delivered_chunks"], 2);
    assert_eq!(details["total_chunks"], 5);
}

#[test]
fn a_stop_before_the_first_chunk_keeps_physical_delivery_unstarted() {
    let details = target_lost()
        .with_details(text_chunk_progress(0, 5))
        .details
        .unwrap();
    assert_eq!(details["physical_delivery_started"], false);
}
