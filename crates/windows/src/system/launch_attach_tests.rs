use super::*;

fn with_args(args: &[&str]) -> LaunchOptions {
    LaunchOptions {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        ..LaunchOptions::default()
    }
}

#[test]
fn a_launch_without_arguments_attaches_to_the_one_running_instance() {
    let mut running = vec![7];

    assert_eq!(attachable(&mut running, &with_args(&[])), Some(7));
}

#[test]
fn a_launch_with_arguments_never_attaches_because_attaching_drops_them() {
    let mut running = vec![7];

    assert_eq!(
        attachable(&mut running, &with_args(&["eventvwr.msc"])),
        None
    );
}

#[test]
fn no_running_instance_means_nothing_to_attach_to() {
    let mut running: Vec<u32> = Vec::new();

    assert_eq!(attachable(&mut running, &with_args(&[])), None);
}
