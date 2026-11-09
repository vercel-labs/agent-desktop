use serde_json::{Value, json};

/// How far a chunked text delivery got when it stopped: the chunks sent, the
/// chunks planned, and whether any input reached the target. The progress is
/// the outer annotation, so it decides `physical_delivery_started` over a
/// lower layer's verdict that saw only its own check.
pub fn text_chunk_progress(delivered_chunks: usize, total_chunks: usize) -> Value {
    json!({
        "delivered_chunks": delivered_chunks,
        "total_chunks": total_chunks,
        "physical_delivery_started": delivered_chunks > 0,
    })
}

#[cfg(test)]
#[path = "text_chunk_progress_tests.rs"]
mod tests;
