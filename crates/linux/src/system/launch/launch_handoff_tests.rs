use super::*;

fn running(pid: u32) -> Running {
    Running {
        executable: None,
        pid,
        instance: format!("boot:{pid}"),
        name: "tool".to_owned(),
    }
}

fn resident(pid: u32, ids: &[&str], aliases: &[&str]) -> Resident {
    Resident {
        owner: running(pid),
        windows: ids.iter().map(|id| (*id).to_owned()).collect(),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
    }
}

fn after(pid: u32, ids: &[&str]) -> (Running, HashSet<String>) {
    (
        running(pid),
        ids.iter().map(|id| (*id).to_owned()).collect(),
    )
}

#[test]
fn one_resident_gaining_a_window_is_the_owner() {
    let before = [resident(7, &["old"], &[]), resident(8, &["other"], &[])];
    let after = [after(7, &["old", "new"]), after(8, &["other"])];
    assert_eq!(
        select_owner(&before, &after, &[]),
        Step::Owner(after[0].0.clone())
    );
}

#[test]
fn a_single_named_resident_with_unchanged_windows_is_the_forwarded_instance() {
    let before = [resident(7, &["old"], &["Files", "org.example.Files"])];
    assert_eq!(
        select_owner(&before, &before_after(&before), &["files".to_owned()]),
        Step::Owner(running(7))
    );
}

fn before_after(before: &[Resident]) -> Vec<(Running, HashSet<String>)> {
    before
        .iter()
        .map(|resident| (resident.owner.clone(), resident.windows.clone()))
        .collect()
}

#[test]
fn a_resident_the_request_does_not_name_is_not_the_forwarded_instance() {
    let before = [resident(7, &["old"], &["Python App", "python-app"])];
    let after = before_after(&before);
    assert_eq!(select_owner(&before, &after, &[]), Step::Wait);
    assert_eq!(
        select_owner(&before, &after, &["Other App".to_owned()]),
        Step::Wait
    );
}

#[test]
fn several_residents_with_unchanged_windows_do_not_prove_a_handoff() {
    let before = [
        resident(7, &["old"], &["Files"]),
        resident(8, &["other"], &["Files"]),
    ];
    let after = before_after(&before);
    assert_eq!(
        select_owner(&before, &after, &["Files".to_owned()]),
        Step::Wait
    );
}

#[test]
fn two_residents_gaining_windows_are_ambiguous() {
    let before = [resident(7, &[], &[]), resident(8, &["old"], &[])];
    let after = [after(7, &["new"]), after(8, &["old", "other"])];
    assert_eq!(
        select_owner(&before, &after, &[]),
        Step::Ambiguous(vec![7, 8])
    );
}

#[test]
fn a_replaced_process_instance_cannot_prove_a_handoff() {
    let before = [resident(7, &["old"], &["Files"])];
    let mut after = vec![after(7, &["new"])];
    after[0].0.instance = "boot:replacement".to_owned();
    assert_eq!(
        select_owner(&before, &after, &["Files".to_owned()]),
        Step::Wait
    );
}

#[test]
fn losing_windows_still_names_the_single_forwarded_instance() {
    let before = [resident(7, &["old", "closed"], &["Files"])];
    let after = [after(7, &["old"])];
    assert_eq!(
        select_owner(&before, &after, &["Files".to_owned()]),
        Step::Owner(after[0].0.clone())
    );
}

#[test]
fn a_process_without_a_captured_baseline_cannot_prove_a_handoff() {
    assert_eq!(select_owner(&[], &[after(7, &["new"])], &[]), Step::Wait);
}

#[test]
fn a_different_pid_with_the_same_instance_token_is_not_a_resident() {
    let before = [resident(7, &["old"], &["Files"])];
    let mut after = vec![after(8, &["new"])];
    after[0].0.instance = before[0].owner.instance.clone();
    assert_eq!(
        select_owner(&before, &after, &["Files".to_owned()]),
        Step::Wait
    );
}
