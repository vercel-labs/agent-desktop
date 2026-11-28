use super::*;
use crate::{ProcessId, ProcessIdentity, Rect};

fn record(pid: u32) -> WindowFrameRecord {
    let frame = Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    WindowFrameRecord {
        process: ProcessIdentity::new(pid, "boot"),
        before: frame,
        maximized: frame,
    }
}

fn memory() -> WindowFrameMemory {
    let root = std::env::temp_dir().join(format!(
        "agent-desktop-frames-{}-{}",
        std::process::id(),
        crate::refs::new_snapshot_id()
    ));
    crate::storage::private_file_parent::ensure_private(&root).unwrap();
    WindowFrameMemory::at(root)
}

#[test]
fn writing_a_record_drops_the_records_of_processes_that_are_gone() {
    let memory = memory();
    memory.remember("gone", record(1), |_| true).unwrap();
    memory
        .remember("live", record(2), |kept| {
            kept.process.pid == ProcessId::new(2)
        })
        .unwrap();
    memory
        .remember("new", record(3), |kept| {
            kept.process.pid != ProcessId::new(1)
        })
        .unwrap();
    assert_eq!(memory.recall("gone").unwrap(), None);
    assert_eq!(memory.recall("live").unwrap(), Some(record(2)));
    assert_eq!(memory.recall("new").unwrap(), Some(record(3)));
}

#[test]
fn a_forgotten_record_is_not_recalled() {
    let memory = memory();
    memory.remember("w", record(2), |_| true).unwrap();
    memory.forget("w").unwrap();
    assert_eq!(memory.recall("w").unwrap(), None);
}

#[test]
fn bookkeeping_after_an_expired_command_deadline_still_writes() {
    let memory = memory();
    let expired = crate::Deadline::detached_after(0).unwrap();
    let _scope = crate::wait::deadline::enter_scope(Some(expired));
    memory.remember("w", record(2), |_| true).unwrap();
    assert_eq!(memory.recall("w").unwrap(), Some(record(2)));
    memory.forget("w").unwrap();
}
