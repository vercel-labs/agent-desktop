use super::*;

#[test]
fn desktop_workflows_keep_batch_parallel_sessions_and_cursor_contracts() {
    let result = get(GetArgs {
        name: "desktop".into(),
        full: false,
        reference: None,
    })
    .expect("embedded desktop skill");
    let content = result["content"]
        .as_str()
        .expect("markdown")
        .replace("\r\n", "\n");
    let workflows = content
        .split("## Workflows\n")
        .nth(1)
        .expect("one compact workflows section")
        .split("\n## ")
        .next()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for guidance in [
        "`batch` runs sequential steps in one process",
        "outer `ok: true` and exit 0 can include failures",
        "Read each entry's `ok`",
        "`--stop-on-error`",
        "immediately before its event `wait`",
        "baseline is captured before the action",
        "`find` when the role or name is known",
        "skeleton snapshot and a `--root` drill",
        "session per task with `session start`",
        "`--session` or `AGENT_DESKTOP_SESSION` to every command",
        "Each agent uses refs from its own snapshots",
        "read-only observations of independent targets can run in parallel",
        "actions take turns under the interaction lease",
        "caller must serialize multi-step edits to the same window",
        "`session start --cursor`",
        "macOS, Windows, and Linux X11, not Wayland",
        "presentation only, sends no input, and needs no `--headed`",
        "`--multi-agent`",
        "distinct `--agent-id` per agent",
        "`trace show`",
        "`trace export`",
        "separate `--stdin` commands, not batch JSON",
        "expose secrets in process arguments",
        "omits observed `value` fields from results",
    ] {
        assert!(workflows.contains(guidance), "missing workflow: {guidance}");
    }
}

#[test]
fn skills_preserve_observation_and_input_damage_boundaries() {
    for (document, guidance) in [
        (SKILL_DESKTOP_MAIN, "fall back to saved values"),
        (
            SKILL_DESKTOP_MAIN,
            "use a fresh snapshot for post-action proof",
        ),
        (SKILL_DESKTOP_MAIN, "not a separate popup"),
        (SKILL_DESKTOP_MAIN, "cannot prove that popup closed"),
        (SKILL_DESKTOP_MAIN, "matching text elsewhere"),
        (SKILL_DESKTOP_MAIN, "miss an already-completed transition"),
        (SKILL_DESKTOP_MAIN, "new whole-window snapshot"),
        (
            SKILL_DESKTOP_MAIN,
            "drilling that unproven ref cannot repair it",
        ),
        (SKILL_DESKTOP_MAIN, "app's own Copy command"),
        (SKILL_LINUX_MAIN, "fix `dc0dc33` before observing"),
        (SKILL_LINUX_MAIN, "abort the target on `Action.GetActions`"),
        (SKILL_LINUX_MAIN, "newline as `Return` and a tab as `Tab`"),
        (SKILL_LINUX_MAIN, "submit a form or move focus"),
        (
            SKILL_LINUX_MAIN,
            "window operations can use a mapped Xwayland client",
        ),
    ] {
        let content = document.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            content.contains(guidance),
            "missing safety boundary: {guidance}"
        );
    }
    assert!(SKILL_DESKTOP_MAIN.lines().count() <= 200);
    for skill in [SKILL_MACOS_MAIN, SKILL_WINDOWS_MAIN, SKILL_LINUX_MAIN] {
        assert!(skill.lines().count() <= 100);
    }
}
