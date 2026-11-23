use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-endpoint-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn each_display_and_state_root_gets_its_own_socket() {
    let runtime = Path::new("/run/user/1000");
    let one = Endpoint::for_display(":0", Path::new("/home/u/.agent-desktop"), Some(runtime));
    let other_display =
        Endpoint::for_display(":1", Path::new("/home/u/.agent-desktop"), Some(runtime));
    let other_root = Endpoint::for_display(":0", Path::new("/tmp/state"), Some(runtime));
    assert_ne!(one.socket, other_display.socket);
    assert_ne!(one.socket, other_root.socket);
    assert!(one.socket.starts_with("/run/user/1000/agent-desktop"));
    assert_eq!(one.lock.with_extension("sock"), one.socket);
}

#[test]
fn without_a_runtime_directory_the_socket_lives_under_the_state_root() {
    let endpoint = Endpoint::for_display(":0", Path::new("/home/u/.agent-desktop"), None);
    assert!(endpoint.socket.starts_with("/home/u/.agent-desktop/cursor"));
    let relative = Endpoint::for_display(":0", Path::new("/s"), Some(Path::new("run")));
    assert!(
        relative.socket.starts_with("/s/cursor"),
        "a relative runtime directory is ignored"
    );
}

#[test]
fn a_directory_others_can_enter_is_refused() {
    let dir = scratch("shared");
    let endpoint = Endpoint::for_display(":0", &dir, None);
    endpoint.ensure_directory().expect("created private");
    let created = endpoint.socket.parent().expect("parent");
    assert_eq!(
        fs::metadata(created)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::set_permissions(created, fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(
        endpoint.ensure_directory().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn only_one_holder_takes_the_lock() {
    let dir = scratch("lock");
    let endpoint = Endpoint::for_display(":0", &dir, None);
    endpoint.ensure_directory().expect("directory");
    let first = endpoint.try_lock().expect("lock").expect("taken");
    assert!(endpoint.try_lock().expect("lock").is_none());
    drop(first);
    assert!(endpoint.try_lock().expect("lock").is_some());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_socket_path_too_long_for_an_address_is_still_reachable() {
    let deep = scratch("deep").join("x".repeat(120));
    let endpoint = Endpoint::for_display(":0", &deep, None);
    assert!(endpoint.socket.as_os_str().len() > MAX_SOCKET_PATH);
    endpoint.ensure_directory().expect("directory");
    let listener = endpoint
        .with_reachable_socket(|path| std::os::unix::net::UnixListener::bind(path))
        .expect("bound");
    assert!(endpoint.socket.exists());
    endpoint
        .with_reachable_socket(|path| std::os::unix::net::UnixStream::connect(path))
        .expect("connected");
    drop(listener);
    let _ = fs::remove_dir_all(deep.parent().expect("parent"));
}
