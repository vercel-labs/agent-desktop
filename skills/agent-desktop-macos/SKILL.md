---
name: agent-desktop-macos
tags: macos-automation, accessibility, ax, tcc, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  macOS setup and troubleshooting for agent-desktop: accessibility permissions,
  native element identity, application-owned surfaces, and headless refusals.
---

# agent-desktop on macOS

Use the core skill's headless observe, act, verify workflow. A platform capability
never authorizes `--headed`; only the user's explicit request does.

## Setup and permissions

The sandbox provider on macOS is `macos`: a macOS VM that needs an Apple Silicon Mac, macOS 13 or later, and Xcode Command Line Tools. See `agent-browser.dev/desktop/sandboxes/macos`.

`agent-desktop status` reports `permissions` and `supported_surfaces`.
Accessibility must be `granted` for observation and actions; Screen Recording
must be `granted` for screenshots, including session screenshot artifacts.
Grant access to the app that launches the CLI and to the binary if listed
separately. Ask the user to change the relevant privacy permissions, then confirm
with `agent-desktop permissions`. A new CLI call sees a new grant; a batch
retains its process's cached report.

`permissions` does not prompt. Requests use a bounded isolated helper.
The permission guide opens a visible settings page and guide panel, so ask the
user before starting it. Automation may report `unknown` when consent would
require a prompt.

## Platform behavior

- Native AX element tokens identify individual controls, including twins. A stale
  token never falls back to a look-alike search. Elements without a token need
  whole-window uniqueness as described in the core skill.
- `menu` and `menubar` observations are application-owned and can omit `data.window`.
  An app without an ordinary window may still expose process-owned surfaces.
  Inspect `supported_surfaces` rather than assuming every surface exists.
- Headless menu-opening and dialog-opening actions can refuse before delivery.
  A label ending in an ellipsis predicts a dialog; it does not prove one exists.
  Stop on a headed-only refusal rather than changing mode.
- A file-name text field is not a file-system rename interface. Value-write
  actions refuse it because a changed field could falsely imply a renamed file.
- Chromium accessibility is enabled by the adapter when needed. A sparse or
  incomplete read still needs another observation, not an assumption of absence.
- AX success alone does not prove an application effect. Verify the task's
  result independently, especially after a menu action or a value write.

## Troubleshooting

Read the disposition before applying a fix. Match both the code and message.

| Code and message pattern | Cause and fix |
| --- | --- |
| `PERM_DENIED`: Accessibility or Screen Recording permission | The launching app lacks the relevant grant. Ask the user to grant it, then check `permissions` in a new CLI call. |
| `POLICY_DENIED`: interaction policy, menu, dialog, or physical route | The operation cannot run headless. Stop and tell the user which operation needs headed mode; do not add `--headed`. |
| `ACTION_NOT_SUPPORTED`: `The field shows a file's name` | A value write cannot prove a rename. Use the app's exposed rename operation or an authorized file-system operation, then verify the file itself. |
| `STALE_REF`: `Element not found` (`resolution_complete_absence`) | The adapter could not land the ref. Take a fresh snapshot in the same session; do not infer delivery from the missing target. |
| `APP_UNRESPONSIVE`: accessibility action did not complete | A modal action may already have landed. If retry is unsafe, inspect the relevant surface and live state without repeating it. |
| `WINDOW_NOT_FOUND`: no window or requested surface | A running process need not own a window. Read `list-windows` and `list-surfaces`; wait for the needed surface instead of activating the app. |
| `PERM_DENIED`: `private file parent is accessible by group or other users` | The state root is not private. Run `chmod 700 "${AGENT_DESKTOP_HOME:-$HOME/.agent-desktop}"`, or use a new state root and let the tool create it. |
| `PERM_DENIED`: `private file parent is not owned by the effective user` | The directory belongs to another user. Point `AGENT_DESKTOP_HOME` at a new directory under your home and let the tool create it; do not take over someone else's state. |
