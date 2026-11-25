/// How far before the capture a process may have started and still count as
/// started after it, for platforms that read start times to the second.
const START_SLACK_MS: u64 = 1_000;

/// The processes that descend from `root` through `parents` (pairs of a process
/// and its parent) and started at or after `since_ms`, in the order found. A
/// descendant that started earlier is walked through, since it can spawn a new
/// one, but is not listed. `started_ms` is read only for descendants, so a
/// platform where a start time costs a handle pays for no other process.
pub fn spawned_descendants_of(
    root: u32,
    since_ms: u64,
    parents: &[(u32, u32)],
    started_ms: impl Fn(u32) -> Option<u64>,
) -> Vec<u32> {
    let mut seen = vec![root];
    let mut spawned = Vec::new();
    let mut next = 0;
    while next < seen.len() {
        let parent = seen[next];
        next += 1;
        for (pid, _) in parents.iter().filter(|(_, owner)| *owner == parent) {
            if seen.contains(pid) {
                continue;
            }
            seen.push(*pid);
            if started_ms(*pid).is_some_and(|started| started + START_SLACK_MS >= since_ms) {
                spawned.push(*pid);
            }
        }
    }
    spawned
}

#[cfg(test)]
#[path = "spawned_tree_tests.rs"]
mod tests;
