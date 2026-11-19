use std::collections::HashMap;
use std::ffi::OsStr;

use super::*;

#[test]
fn only_a_binary_that_enters_the_renderer_starts_one_whatever_its_name() {
    let npm = || {
        Ok(PathBuf::from(
            "/lib/node_modules/agent-desktop/bin/agent-desktop-linux-x64",
        ))
    };
    assert_eq!(
        image_for(true, npm()),
        Some(PathBuf::from(
            "/lib/node_modules/agent-desktop/bin/agent-desktop-linux-x64"
        ))
    );
    assert_eq!(image_for(false, npm()), None);
    assert_eq!(
        image_for(false, Ok(PathBuf::from("/usr/local/bin/agent-desktop"))),
        None
    );
    assert_eq!(
        image_for(true, Err(std::io::Error::other("no image"))),
        None
    );
}

#[test]
fn the_renderer_gets_an_allowlisted_environment_and_the_chosen_display() {
    let parent: HashMap<&str, &str> = [
        ("DISPLAY", ":7"),
        ("HOME", "/home/u"),
        ("AWS_SECRET_ACCESS_KEY", "x"),
    ]
    .into();
    let command = build_command(Path::new("/bin/agent-desktop"), ":3", &|name| {
        parent.get(name).map(|value| (*value).to_owned())
    });
    let env: HashMap<&OsStr, Option<&OsStr>> = command.get_envs().collect();
    assert_eq!(
        env.get(OsStr::new("DISPLAY")),
        Some(&Some(OsStr::new(":3")))
    );
    assert_eq!(
        env.get(OsStr::new(DISPLAY_ENV)),
        Some(&Some(OsStr::new(":3")))
    );
    assert_eq!(
        env.get(OsStr::new(CHILD_MARKER)),
        Some(&Some(OsStr::new(GENERATION)))
    );
    assert_eq!(
        env.get(OsStr::new("HOME")),
        Some(&Some(OsStr::new("/home/u")))
    );
    assert!(!env.contains_key(OsStr::new("AWS_SECRET_ACCESS_KEY")));
    let args: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(args, vec![OsStr::new(CHILD_ARGUMENT)]);
}
