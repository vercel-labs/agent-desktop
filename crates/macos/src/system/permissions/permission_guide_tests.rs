use super::*;
use std::time::Instant;

fn detached_sleep() -> i32 {
    let launched = Command::new("/bin/sh")
        .args(["-c", "/bin/sleep 30 >/dev/null 2>&1 & echo $!"])
        .output()
        .unwrap();
    let orphan: i32 = String::from_utf8(launched.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let give_up = Instant::now() + Duration::from_secs(5);
    while parent_pid(orphan) != Some(1) && Instant::now() < give_up {
        std::thread::sleep(Duration::from_millis(5));
    }
    orphan
}

#[test]
fn guide_covers_only_missing_permissions() {
    assert_eq!(missing_steps(true, true), 0);
    assert_eq!(missing_steps(false, true), ACCESSIBILITY);
    assert_eq!(missing_steps(true, false), SCREEN_RECORDING);
    assert_eq!(
        missing_steps(false, false),
        ACCESSIBILITY | SCREEN_RECORDING
    );
    assert_eq!(parse_steps("3"), Some(3));
    assert_eq!(parse_steps("0"), None);
    assert_eq!(parse_steps("4"), None);
}

#[test]
fn the_guide_lock_stays_held_from_the_check_through_the_child() {
    let path = std::env::temp_dir().join(format!("{LOCK_NAME}.{}.test", std::process::id()));
    let lock = lock_file(&path).unwrap().unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    hand_over(&mut command, &lock);
    let mut child = command.spawn().unwrap();
    drop(lock);
    assert!(lock_file(&path).unwrap().is_none());
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(lock_released_within(&path, Duration::from_secs(10)));
    std::fs::remove_file(&path).unwrap();
}

fn lock_released_within(path: &Path, limit: Duration) -> bool {
    let give_up = Instant::now() + limit;
    loop {
        if lock_file(path).unwrap().is_some() {
            return true;
        }
        if Instant::now() >= give_up {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_guide_offers_the_outermost_app_bundle_of_the_launching_process() {
    let rex = Path::new(
        "/Users/me/Library/Application Support/com.superlogical.rex/RexServer/Versions/Current/Rex Server.app/Contents/MacOS/rex",
    );
    assert_eq!(
        outermost_app(rex),
        Some(PathBuf::from(
            "/Users/me/Library/Application Support/com.superlogical.rex/RexServer/Versions/Current/Rex Server.app"
        ))
    );
    let code_helper = Path::new(
        "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin)",
    );
    assert_eq!(
        outermost_app(code_helper),
        Some(PathBuf::from("/Applications/Visual Studio Code.app"))
    );
    assert_eq!(
        outermost_app(Path::new(
            "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"
        )),
        Some(PathBuf::from("/System/Applications/Utilities/Terminal.app"))
    );
    assert_eq!(
        outermost_app(Path::new("/usr/local/bin/agent-desktop")),
        None
    );
}

#[test]
fn the_host_walk_passes_through_a_process_of_another_user() {
    let parent = parent_pid(i32::try_from(std::process::id()).unwrap());
    assert_eq!(parent, Some(unsafe { libc::getppid() }));
    let launchd_child = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,uid="])
        .output()
        .unwrap();
    let root_child = String::from_utf8(launchd_child.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let fields: Vec<i32> = line
                .split_whitespace()
                .filter_map(|field| field.parse().ok())
                .collect();
            (fields.len() == 3 && fields[0] > 1 && fields[1] == 1 && fields[2] == 0)
                .then_some(fields[0])
        })
        .next()
        .unwrap();
    assert_eq!(parent_pid(root_child), Some(1));
}

#[test]
fn the_host_survives_a_process_detaching_from_its_launcher() {
    let orphan = detached_sleep();
    let own = i32::try_from(std::process::id()).unwrap();
    let orphan_responsible = responsible_pid(orphan);
    let own_responsible = responsible_pid(own);
    unsafe { libc::kill(orphan, libc::SIGKILL) };
    let inherited = orphan_responsible.is_some() && orphan_responsible == own_responsible;
    let nothing_to_inherit = own_responsible == Some(own) && orphan_responsible == Some(orphan);
    assert!(
        inherited || nothing_to_inherit,
        "orphan {orphan} answers {orphan_responsible:?}, this process {own} answers {own_responsible:?}"
    );
}

#[test]
fn the_responsible_app_wins_over_the_ancestor_walk() {
    let listing = Command::new("/bin/ps")
        .args(["-axo", "pid="])
        .output()
        .unwrap();
    let (app_pid, bundle) = String::from_utf8(listing.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.trim().parse::<i32>().ok())
        .find_map(|pid| app_of(pid).map(|bundle| (pid, bundle)))
        .unwrap();
    let orphan = detached_sleep();
    let with_responsible = host_app_of(orphan, |_| Some(app_pid));
    let without = host_app_of(orphan, |_| None);
    let outside_a_bundle = host_app_of(app_pid, |_| Some(orphan));
    unsafe { libc::kill(orphan, libc::SIGKILL) };
    assert_eq!(with_responsible, Some(bundle.clone()));
    assert_eq!(without, None);
    assert_eq!(outside_a_bundle, Some(bundle));
}
