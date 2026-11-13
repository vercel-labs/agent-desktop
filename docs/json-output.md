# JSON output contract

Every command returns structured JSON:

```json
{
  "version": "2.7",
  "ok": true,
  "command": "click",
  "data": {
    "action": "click",
    "disposition": { "delivery": "delivered_unverified", "retry": "unsafe" }
  }
}
```

Errors include machine-readable codes and recovery hints:

```json
{
  "version": "2.7",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "@s8f3k2p9:e7 not found in current RefMap",
    "suggestion": "Re-run a snapshot to obtain fresh refs, then retry with the new ref (CLI: snapshot [--skeleton]; FFI: ad_snapshot then pass the printed qualified ref to ad_execute_by_ref)",
    "recovery": {
      "strategy": "refresh_snapshot_then_retry_original",
      "retryable": true,
      "requires_fresh_snapshot": true
    },
    "disposition": {
      "delivery": "not_delivered",
      "retry": "safe"
    }
  }
}
```

`error.code`, `error.message` and `error.disposition` are always present. `suggestion`, `recovery`, `platform_detail` and `details` are omitted when empty. `recovery` appears only when `disposition.retry` is `safe`, and its `retry_after_ms` is set only for `TIMEOUT` and `APP_UNRESPONSIVE`.

Version 2.2 replaces the 2.1 `TIMEOUT` error for a budget-exhausted snapshot
with a successful, explicitly partial response, and adds `data.complete`.
Snapshot data always carries `complete`. A snapshot that runs out of budget now
returns `ok: true` with `"complete": false`, the tree it did observe,
`"truncated": true`, and `nodes_observed`, instead of a `TIMEOUT` error and no
tree. Every node whose descendants were cut short carries
`"subtree_truncated": true`, so a reader can walk from the root to each
boundary. Consumers must read `data.complete` to decide whether a tree is whole;
a `TIMEOUT` error no longer signals an oversized tree. A `--root` drill-down is
unaffected: it replaces refs inside an existing snapshot, so an incomplete
observation still returns `TIMEOUT` rather than a partial tree.

Version 2.1 replaces the 2.0 `error.retry_command` string with the structured
`error.recovery` object and adds `error.disposition`. Consumers must select a
recovery strategy from `recovery.strategy` only when `disposition.retry` is
`safe`; command strings from older envelopes must not be executed blindly.
The removed `retry_command` field has no compatibility alias.

## Sandbox results

`sandbox install` returns `data: {"providers": [...]}` and `sandbox list` returns `data: {"sandboxes": [...]}`. An entry has `name`, `state`, and `message`, with `next` when a further action is available; a sandbox entry, and the `data` of a single sandbox command, also carry `provider`. Provider-specific fields may appear beside these. `sandbox install <provider>` returns that entry as `data`. A build that includes no providers returns an empty `providers` array, and a command that names a provider fails with `INVALID_ARGS`. States are `unavailable`, `missing`, `ready`, `working`, `needs_setup`, `running`, `stopped`, and `broken`.

A sandbox whose record is missing, malformed or unreadable, or names a provider this build does not include, is listed as `broken`, and every command on it returns `INVALID_ARGS`. agent-desktop has no provider to ask, so `next` and the error's `suggestion` name the sandbox directory to delete once nothing in it is still running. When the provider's own status check fails or times out, the entry is also `broken` and `next` carries that error's suggestion.

A `sandbox create` that times out, or whose provider ends without a readable reply, keeps its record, because the provider may have made something. The sandbox stays in `sandbox list`; run `sandbox status <name>` to see what exists and `sandbox rm <name>` before creating that name again. A create the provider refuses leaves no record unless the provider left files in the sandbox directory. `create` never takes over a directory that already holds files.

`create`, `start`, `stop` and `rm` on one sandbox run one at a time. A second one fails at once with `ACTION_FAILED` and `not_delivered`.

`sandbox exec` passes the guest command's stdout and exit status through unchanged instead of adding an envelope; the guest's stderr is not passed through and the guest receives no stdin, so a guest command that reads `--stdin` gets empty input. A provider failure returns a host error envelope instead. agent-desktop reads at most 8 MiB from each of the guest's output streams; a command that prints more fails with `ACTION_FAILED`, so write large output to a file in the guest. A guest command that runs longer than 300 seconds fails with `TIMEOUT`. The host's session and agent id never reach the guest: the flags are refused before `sandbox exec`, and `AGENT_DESKTOP_SESSION` and `AGENT_DESKTOP_AGENT_ID` are not passed on. Give the guest command its own `--session` after `--`.

## Disposition values

Every error carries `disposition`, and commands that act on a target may also
carry it on their result. `delivery` is one of `unknown`, `not_delivered`,
`delivery_uncertain`, `delivered_unverified` or `delivered_verified`. `retry`
is one of `unknown`, `safe` or `unsafe`, and it follows from `delivery`:
`not_delivered` is `safe`, and every other non-`unknown` value is `unsafe`.
No result pairs `delivered_verified` with `retry: "safe"`: even a batch whose
verified work sits beside never-dispatched failures re-reads a fresh listing
on a re-run, so its retry stays `unsafe`.

`unknown` means the command established nothing about delivery. A failure of a
mutating command that carries no delivery evidence, such as a `TIMEOUT` on an
operation that may already have landed, keeps it. A read-only command sends
nothing and invalid arguments never reach the application, so their errors, and
a lookup failure such as `APP_NOT_FOUND`, carry `not_delivered`. Treat `unknown`
as not safe to retry automatically: read `error.code` and the message, and
choose a recovery strategy only when `disposition.retry` is `safe`.

## Version 2.4 migration

Stateful ref actions (`set-value`, `type`, `clear`, `check`, `uncheck`, `toggle`, `expand`, and `collapse`) now reject observed postcondition contradictions after one delivery attempt. Readback failures preserve the adapter's error code and carry `details.kind: "post_action_verification"`. Missing evidence alone remains a successful `delivered_unverified` result with `details.verification_scope: "unavailable"`; neither that result nor verified element state proves application-level persistence.

macOS mutation errors that do not prove non-delivery stop fallback chains with an unsafe retry disposition. This affects activation, selection, disclosure, scrolling, and window operations; an unchanged or unreadable state is not proof that a mutation was never delivered. Value-writing chains also stop after unverified delivery instead of applying a second mechanism. Failures after preparatory scrolling carry `details.preparatory_scroll`, which records that the view may have changed. Their retry disposition follows the failure: a refusal that sent nothing is `not_delivered` and safe to retry, and any other failure keeps its own disposition.

Inspect `disposition.retry` before retrying or choosing an alternative action, regardless of `error.code`. An error may include `post_state` and `postcondition_satisfied` without erasing the original execution failure. The same verification rules apply to FFI ref actions; the C ABI is unchanged. The Rust `ActionResult::from_execution` constructor now takes only action and steps and returns an infallible result; live verification belongs to `execute_verified_action`.

## Version 2.5 migration

Version 2.5 is a breaking payload change for the commands that dispatch pointer, drag, clipboard, app-close and window operations: `mouse-click`, `mouse-move`, `mouse-wheel`, `hover`, `drag`, `clipboard-set`, `clipboard-clear`, `close-app`, `move-window`, `resize-window`, `minimize`, `maximize` and `restore`.

The unconditional success flags are removed with no compatibility alias: `clicked`, `moved`, `scrolled`, `hovered`, `dragged`, `resized`, `minimized`, `maximized`, `restored`, `closed`, `cleared`, the `requested` field of `close-app`, and the inner `ok` of `clipboard-set`. A flag that every successful call carried proved nothing about the operation.

A verified window report names the exact readback in `details.verification_scope` (`window_size`, `window_position`, `window_minimized_state` or `window_maximized_state`). Geometry scopes also state `details.tolerance` and `details.units`: 2 points on macOS, 8 px on Windows, 2 pixels on Linux. A window failure after the write keeps its code, details and disposition and gains `details.target` and the completed `details.steps`.

Each of these commands now returns the adapter's own report inside `data`:

```json
{
  "action": "drag",
  "disposition": { "delivery": "delivered_unverified", "retry": "unsafe" },
  "steps": [ { "label": "pointer_drag", "outcome": "succeeded", "mechanism": "physical_synthetic" } ],
  "target": { "kind": "drag", "from": { "x": 10, "y": 20 }, "to": { "x": 80, "y": 20 } },
  "from": { "x": 10, "y": 20 },
  "to": { "x": 80, "y": 20 }
}
```

- `disposition.delivery` is `delivered_verified` only after the adapter read back the postcondition it names, `delivered_unverified` after a completed dispatch without that proof, and `not_delivered` only when the adapter observed that the operation was already satisfied (for example `close-app` on an application that had already exited). `ok: true` means the command returned a delivered or already-satisfied result, not that the application effect was verified.
- `target.kind` is `window` (`id`, `title`, `process`), `app` (`name`, `process`), `pointer` (`point`), `drag` (`from`, `to`) or `clipboard`. The adapter supplies the identity it validated and used. A pointer target is the dispatch coordinate and does not prove which application received the input.
- Request context (`x`, `y`, `count`, `dx`, `dy`, `width`, `height`, drag endpoints, `drop_delay_ms`, `type`, `app`, `method`, and a confirmed `focused`) keeps its earlier meaning. These fields echo the request and are not observed post-state.
- Errors keep `error.disposition`. A failed `--wait-for` after a delivered operation keeps the operation's own delivery (verified, unverified, or not delivered) in `error.disposition` and carries the report under `details.after_action`.
- The Rust `ActionResult` and `OperationReport` readers reject a payload without an explicit `disposition`; a missing disposition no longer reads back as delivered-unverified.
- The C ABI is unchanged. Status-only functions such as `ad_close_app` and `ad_drag` still cannot expose the disposition, steps or target of a successful call.

## Version 2.6 migration

Version 2.6 changes which refs resolve on Windows and Linux, and on macOS for an element that yields no address. Such a ref resolves only when one complete read of its window proved it unique when it was captured and exactly one live element matches its role and stable fields now. The only new payload fields are `data.identity_unproven` and, on a ref action whose budget ran out after a refusal, `error.details.refusal`.

- A ref to one of two identical controls, which earlier resolved by position or geometry, now fails `AMBIGUOUS_TARGET` with `details.kind: "identity_not_unique"` before anything is read. So does a ref captured from a window that could not be read whole within the snapshot timeout.
- A refmap written before 2.6 records no capture proof, so all of its refs on these platforms fail the same way. Take a new snapshot.
- An identical twin that appears after capture fails `AMBIGUOUS_TARGET` with `details.kind: "saved_ref_ambiguous"`.
- All of these carry `details.retryable: false` and a `not_delivered` disposition. `disposition.retry` stays `safe` because nothing was sent, but repeating the same ref returns the same refusal.
- When a ref action's full resolution under the lease is refused in a way that may clear, the action waits again within its `--timeout-ms`. If that budget then runs out, the error carries the refusal in `details.refusal`: its `code`, `message`, and `details` when the refusal had any.
- A surface kind the OS does not have now returns `INVALID_ARGS` instead of `PLATFORM_NOT_SUPPORTED`, with the disposition `not_delivered` and `retry: "safe"`. FFI callers see the matching `AdResult` code change. An adapter with no surfaces at all still returns `PLATFORM_NOT_SUPPORTED`.
- On macOS, `open-system-surface --surface dock` and `--surface menu-bar-extras` now succeed headless. Their `data` has no `window`; check for the field before reading it. Windows and Linux still return `window` and need `--headed`.
- On macOS, a snapshot of a background app that lists no window (a menu-bar app, the Dock, Control Center), and `--surface dock` or `--surface menu-bar-extras`, now succeed and read the app through its process. Earlier they failed `WINDOW_NOT_FOUND`. `data.window` is omitted from such a snapshot and `data.app` names the process; check for the field before reading `data.window.id`.

A snapshot that needed one complete read of its window to prove its refs, and could not get it, says so with `"identity_unproven": true` in `data`. `complete` still describes the presented tree. On Windows and Linux, and on macOS for an element with no address, every ref of that snapshot fails `AMBIGUOUS_TARGET` with `details.kind: "identity_not_unique"`; retake the snapshot with a larger `--timeout-ms`. The field is omitted otherwise.

On macOS, application-owned `menu` and `menubar` snapshots omit `data.window`. `data.app` identifies the owner, including for a `--root` drill into a ref from one of these surfaces. Window-owned snapshots retain `data.window`. When `--window-id` is supplied for an application-owned surface, it selects the owning app while the observed surface remains application-owned.

A complete macOS menu lookup with no surface returns `ELEMENT_NOT_FOUND` with `details.kind: "surface_absent"`, `details.surface: "menu"` or `"menubar"`, and `details.complete: true`. That is completed absence evidence. An absent `menubar` can mean the application exposes no menu bar. A `menu` lookup requires an open menu: snapshot `menubar` when it exists, click its owning item without `--headed`, then snapshot `--surface menu`. Missing `complete`, `complete: false`, a timeout, or an incomplete AX read leaves the result unknown, not absent. If a prior action has `delivery_uncertain` and `retry: unsafe`, inspect the current UI before any action; do not repeat the action.

- Focus and preparatory scrolling can change coordinates. Stability is verified against fresh live observations, not by requiring coordinates to remain equal to the original snapshot.
- On macOS, clicking an item in the native menu bar uses `AXPress`. It works headless, and under `--headed` it does not move the pointer.

- Focusing an inactive text field can change its selection. A pre-focus range cannot establish the expected insertion into a nonempty field; such a result is explicitly unverified rather than falsely contradicted.
- A settle-loop pacing budget does not replace the caller's deadline for native reads. Tests assert deadline identity rather than arbitrary remaining-time thresholds.

### Post-action observation

When a ref action returns `delivery_uncertain` with a post-action wait selector, `error.details.post_action_observation` records a read-only observation of the original process. Application-owned refs keep their source surface. Window-owned refs observe their original pinned window, including refs captured from `sheet`, `alert`, `popover`, `focused`, or a window-owned `menu` such as a Linux menu. This lets a closed overlay be observed without treating the parent window as absent. The action is never repeated, and its original error and unsafe retry disposition stay unchanged. Even `expected_state_observed` does not prove that the action caused the state. Observation uses `--wait-timeout` separately from the action timeout.

| Field | Meaning |
| --- | --- |
| `kind` | Always `post_action_observation`. |
| `selector, expected` | Requested selector and `present` or `absent`. |
| `surface` | The observed scope: the application-owned source surface, or `window` for a window-owned ref. |
| `process_identity` | Original `pid` and `process_instance`; the instance can be null for `identity_unavailable`. |
| `elapsed_ms` | Time spent observing after the action returned. |
| `source_window_id` | Original window ID, when the ref has one. |
| `selector_matches` | Observed selector result, when available; absence requires a complete read. |
| `process_state` | Last observed `unresponsive`, `exited`, or `crashed` state. `running` is never emitted. |
| `last_observation_error` | Last observation error's `code` and `message`, when available. |
| `outcome` | One of the outcomes below. |

| Outcome | Meaning |
| --- | --- |
| `expected_state_observed` | The expected selector state was observed, or the original window or surface was confirmed absent for an absence wait. |
| `process_exited` | The original process exited; this does not establish the action's effect. |
| `process_crashed` | The platform reported that the original process crashed. |
| `owner_unavailable` | The original surface owner remained unavailable without confirmed process exit. |
| `observation_failed` | A terminal read error stopped observation; inspect `last_observation_error`. |
| `timed_out` | The wait budget expired without observing the expected state, including when transient read errors persisted through the last poll. `last_observation_error` may hold the last error. |
| `identity_unavailable` | The ref has no usable process generation, so observation was skipped. |

## Version 2.7 migration

- Headed-mode refusal suggestions require an explicit user request for headed operation; agents must not add `--headed` to recover from a headless failure.
- Private-file privacy errors keep their code and add `details.path` and a fix: Unix permissions (`chmod 700` for a directory) or Windows permissions limited to this user, SYSTEM and Administrators, or a new `AGENT_DESKTOP_HOME`.
- `status` adds `data.state_root_privacy`: `{ "ok": true }` or `{ "ok": false, "error": { ... } }` from the same private-directory check; failed checks leave snapshot rows null.

`launch`, `open-system-surface`, `focus-window`, `dismiss-notification` and `dismiss-all-notifications` now carry `data.disposition` on success, and an incomplete-locator `TIMEOUT` names its cause. Every key is additive; every other key keeps its value. A `launch` payload written before the field existed has no `disposition`; readers must treat its absence as `unknown`, not as a delivery claim.

- `launch` reports `not_delivered` (`retry: "safe"`) when it attached to a running application and dispatched nothing — no `--activate`, or an `--activate` whose target was already active and needed no request. With `--activate` it reports `delivered_verified` only when the platform read back the target as the frontmost application afterwards, and `delivered_unverified` (`retry: "unsafe"`) otherwise. A fresh launch reports `delivered_verified` when the adapter read back the process it started, and `delivered_unverified` when it did not — including a spawned process that never appeared on the platform's accessibility bus.
- `open-system-surface` reports `not_delivered` when the surface was already presented and the adapter raised nothing, and `delivered_verified` when it raised the surface and then observed it presented. On macOS the surfaces are always presented, so the command always reports `not_delivered`.
- `focus-window` reports `delivered_verified`: it returns `Ok` only after the OS reports the window focused on two consecutive reads.
- `dismiss-notification` reports `delivered_verified`: the adapter returns `Ok` only after re-reading that the entry is gone from the surface.
- `dismiss-all-notifications` derives `data.disposition` from each entry's own delivery evidence, not from counts: `not_delivered` (`retry: "safe"`) when nothing was found or every failure happened before anything was sent; `delivered_unverified` (`retry: "unsafe"`) when any entry's dispatch started; and `delivered_verified` (`retry: "unsafe"`) when every captured entry was dismissed or the remaining failures were all never dispatched — the `failures` list then names what a re-run still has to touch, and the retry stays unsafe because a re-run captures a fresh listing. An entry that vanished before its turn counts as dismissed.
- A `TIMEOUT` from `find` and the other locator commands whose search could not prove its result carries `details.cause`: `deadline` (the search ran out of time; `details.retryable` is `true`), `unreadable_elements` (some native reads failed, so part of the tree is unknown; `details.retryable` is `false` for search kinds and `true` for selected-match hydration unless a traversal limit was hit), or `traversal_limit` (a fixed traversal limit was hit; `details.retryable` is `false`). When retrying cannot help, narrow the query with `--role` or `--name`, or run `snapshot`, which returns the readable part of the tree with `complete: false`.

## Find result shape

`find` match objects report the element's actual accessibility `name`. Elements
without an accessible name omit `name` entirely; their content stays in `value`.
Earlier output synthesized a display name, falling back to `value` or
`description`, then a `(unnamed <role>)` placeholder, so consumers that treated
`name` as always present must tolerate its absence and apply their own fallback
to `role` or `value`.

## Ref re-identification details

Every ref is re-identified from its stored entry or refused, and `details.kind` names which. The core resolver refuses a saved ref with one of these five kinds:

| `details.kind` | Code | Meaning |
|------|------|---------|
| `retained_object_stale` | `STALE_REF` | The element's recorded address is dead or no longer names it. macOS: its process, role or recorded identifier no longer matches; `details.reason` is `dead`, `pid`, `role`, `native_identifier`, or `legacy` for a ref saved by an earlier release whose address can no longer be checked. Linux (GTK and Chromium apps): `details.reason` is `bus` (the accessibility bus restarted), `window` (the ref's window is another one now), `chain` (a parent no longer lists its recorded child; `details.depth` is the link, 0 for the application root, which drops a closed window) or `identity` (the element's role, stable name, description or native id changed). No search runs, so the ref cannot land on a look-alike. Take a fresh snapshot. |
| `saved_ref_absent` | `STALE_REF` | No live element matches the stored role and stable fields over complete evidence, so the element is gone. Take a fresh snapshot. |
| `saved_ref_ambiguous` | `AMBIGUOUS_TARGET` | An element identical to the captured one appeared, so two or more live elements match the stored role and stable fields. A fresh snapshot refuses both as twins; act on a control that is unique in the window. |
| `identity_not_unique` | `AMBIGUOUS_TARGET` | The ref was not proven the only element matching its role and stable fields when it was captured, so nothing is searched. Either an identical twin existed, or the window could not be read whole within the snapshot timeout (retake the snapshot with a larger `--timeout-ms`). It is reached on Windows, on Linux for Qt and other toolkits, and on macOS when the element yields no address. A twin is refused by every snapshot; only an explicit coordinate click (`--headed mouse-click --xy`) can pick one, it takes the foreground, and it is unavailable on a Linux Wayland session. |
| `element_token_required` | `STALE_REF` | The platform re-identifies this ref only through the element token captured with it, and the ref carries none it can read. Nothing is searched, so this does not say the element is gone. Take a fresh snapshot and use the new ref |

The sixth outcome is `resolution_identity_unknown`, which carries `TIMEOUT`: a live field could not be read, so the identity count stayed unproven. It is retryable. A large window can need more time than the budget allows, so retry the same command with a larger `--timeout-ms`. Linux menu and tray refs report the same kind as `STALE_REF`.

After the core resolver chooses the element, the platform adapter lands it, and a landing failure keeps the adapter's own `details.kind` (for example `resolve_no_candidate` on Linux and Windows, or `resolution_complete_absence` on macOS). The platform skills list those kinds. `saved_ref_absent`, `saved_ref_ambiguous`, `identity_not_unique`, `retained_object_stale` and `element_token_required` carry `details.retryable: false`, because waiting cannot change the answer. Every one of these is `not_delivered`.

## Error codes

| Code | Meaning |
|------|---------|
| `PERM_DENIED` | Accessibility permission not granted |
| `ELEMENT_NOT_FOUND` | No element matched the ref or query |
| `APP_NOT_FOUND` | No matching application process; the lookup failed before anything was sent, so the disposition is `not_delivered` |
| `STALE_REF` | Ref could not be re-identified in the live UI |
| `AMBIGUOUS_TARGET` | Ref recovery matched multiple plausible targets |
| `SNAPSHOT_NOT_FOUND` | Snapshot ID is missing or expired |
| `SANDBOX_NOT_FOUND` | No sandbox has that name; run `agent-desktop sandbox list` to see the sandboxes that exist |
| `POLICY_DENIED` | The interaction policy does not permit an effect of the operation; nothing was sent. `details.effect` names it when one effect decided: `focus_steal`, `cursor_move` or `clipboard_write`. A refusal of an effect on the user's foreground also carries `details.check: "foreground"` and `details.foreground_effect: true`. Before dispatch, `details.mechanisms` is the action name and `details.kind` is the reason: `shows_menu`, `opens_dialog`, `opens_popup` or `focus`. When a delivery route is held back, `details.mechanisms` names the withheld route labels. A ref action whose remaining routes all deliver nothing, while one was held back, is `POLICY_DENIED`, not `ACTION_FAILED`, on every platform. Rerun with `--headed` to permit them |
| `ACTION_FAILED` | Action rejected, or its observed result contradicted the request |
| `ACTION_NOT_SUPPORTED` | The target does not expose the requested action |
| `APP_UNRESPONSIVE` | The matching application stopped responding |
| `PLATFORM_NOT_SUPPORTED` | Adapter method not implemented on this platform |
| `TIMEOUT` | Wait condition expired, or a live identity read stayed incomplete until the deadline. Incomplete locator errors carry a `details.cause`; see below |
| `INVALID_ARGS` | Invalid argument values |
| `WINDOW_NOT_FOUND` | The requested window does not exist, including a `screenshot --window-id` that matches no window; the disposition is `not_delivered` |
| `NOTIFICATION_NOT_FOUND` | The notification ID was not found |
| `INTERNAL` | Unexpected error or caught panic |

### Locator incomplete

`find` and the other locator commands return `TIMEOUT` when the search could not prove its result. These `details.kind` values carry `details.cause`: `locator_incomplete`, `locator_budget_limit`, `locator_transient_incomplete`, `locator_selected_query_incomplete`, `locator_selected_query_budget_limit`, and `locator_selected_evidence_incomplete`. `details.cause` says why:

| `details.cause` | Meaning | `details.retryable` |
|------|---------|------|
| `deadline` | The search ran out of time. Retry with a larger `--timeout-ms`. | `true` |
| `unreadable_elements` | The search finished in time, but some native reads failed, so part of the tree is unknown. | `false` for search kinds (`locator_incomplete`, `locator_budget_limit`); `true` for selected-match hydration (`locator_selected_query_incomplete`, `locator_selected_evidence_incomplete`) unless a traversal limit was hit |
| `traversal_limit` | A fixed traversal limit (node, edge, child, text or depth) was hit. More time does not help. | `false` |

When `details.retryable` is `false`, narrow the query with `--role` or `--name`, or run `snapshot`, which returns the readable part of the tree with `complete: false`. When `details.cause` is `unreadable_elements` on selected-match hydration without a traversal limit, run the command again. An exhausted operation deadline takes precedence over read failures, which take precedence over traversal limits. An incomplete result with no recorded cause is `unreadable_elements`. `details.query_stats` and the kind's additional fields carry the evidence. The disposition is `not_delivered`. A selector wait that expires preserves the locator's `kind` and `cause` in `details.last_error` under its `wait_timeout` error.

## Exit codes

`0` success, `1` structured error (JSON on stdout), `2` command-line parse error (the envelope is still printed, with `INVALID_ARGS`). A parse error's `error.suggestion` carries clap's `tip:` lines and the command's `Usage:` line when clap printed them, then, when the command is known, a pointer to `agent-desktop <command> --help`.

## Planned 2.0 vocabulary

The 2.7 envelope keeps every current name. 2.0 is a future breaking release that will use one name per concept.

| concept | names in 2.7 (per command) | one name in 2.0 |
| --- | --- | --- |
| Application name | `name` in list-apps and close-app `target`; `app_name` in window and notification records; `app` in launch, snapshot, wait events and details, and the close-app request echo | `app` on app-owned records; `name` on the app record itself |
| Application metadata | `bundle_id`, `presentation` in list-apps; absent from launch, windows, notifications, and events | No rename specified |
| Process identity | Flat `pid`, `process_instance` in list-apps, list-windows, and launch; `pid` alone in wait events and list-surfaces; `target.process.{pid,instance}` in close-app and window operations; `process_identity.{pid,process_instance}` in post-action observation; neither in snapshot data | Flat `pid`, `process_instance`; omit an unknown instance |
| Window identity | `id`, `title` in window records, operation targets, and snapshot; `window_id`, `title` in wait events; `source_window_id` in post-action observation; snapshot includes only `id`, `title` | Record `id`, `title`; reference `window_id`; full window record in snapshot |
| Window container | `window` in launch, snapshot, wait --window, and open-system-surface; `focused` object in focus-window | `window` |
| Focus state | `is_focused` in window records; `focused` element token; `result` in is focused; `focused` boolean in hover and drag | Window `focused`; element token `focused`; hover and drag `window_focused` |
| Element state | `states` tokens in snapshot and find; get --property states returns tokens in `value`; ref-action `post_state` has `states` plus `enabled`, `hidden`, `offscreen` booleans | `states` tokens, including `enabled` when evidenced |
| Predicate answer | `property`, `result`, `applicable` in is; wait --element `observed` uses `exists`, `enabled`, `visible`, `actionable`, or `matched` with value evidence | No common name specified |
| Element ref | `ref_id` in snapshot and find; `ref` in get, is, wait, and trace events | `ref_id`, qualified |
| Element name | `name` in snapshot and find; get --property title reads the element name | `name`; keep `title` as an alias for one minor release |
| Absent value | Find emits null `ref_id`, null `value`, and empty `states`; status emits null `snapshot_id`, `ref_count`, `session_id`; wait --text emits null `ref`, `role`; snapshot omits absent node fields and empty arrays | Omit absent fields and empty arrays |
| List wrapper | `{apps}` in list-apps; bare arrays in list-windows and list-displays; `{pid,surfaces}` in list-surfaces; `{count,notifications}` in list-notifications; `{sessions}` in session list | Named arrays: `apps`, `windows`, `displays`, `surfaces`, `notifications`, `sessions`; each wrapper has `complete`; surfaces also has `pid`, `process_instance` |
| Counts | `count` in list-notifications, find --count, wait --text, and mouse-click; `total_matches` in find; `ref_count` in snapshot and status; `dismissed_count`, `failed_count` in dismiss-all-notifications; `event_count`, `total_events`, `returned_events` in trace export | `count` for a sibling array's length; find --count `total_matches`; wait --text `match_count`; mouse-click `click_count`; drop `event_count` |
| Partial results | Conditional `complete:false`, `skipped` in list-apps; `complete`, conditional `truncated`, `nodes_observed` in snapshot; `truncated` in find and trace; no flag in list-windows | Always `complete`; `truncated` when cut |
| Timing | `elapsed_ms` in conditional waits, snapshot --wait-for, and batch; requested `waited_ms` in wait MS; `lease_hold_ms` in ref-action details; `created_at`, `ended_at` in session output | Measured `elapsed_ms` for every wait; session output `created_at_ms`, `ended_at_ms`; stored timestamps keep their names |
| Wait success | `found` in menu, window, text, element, and event waits; `matched`, `condition` in notification waits; `matched_selector`, `gone`, `target_absent` for selector absence; snapshot fields for selector presence and --wait-for | `found` plus `condition`; keep `matched_selector` |
| Event spelling | Input focus-changed emits `focus_changed_window`; other wait event kinds use underscores; event surfaces use `action_center`, while open-system-surface uses `action-center` | `focus_changed`; kebab-case surface names |
| Kind key | `type` in list-surfaces, ref-action surface records, and clipboard-get; `kind` in operation targets, events, and details; `surface` in surface events and open-system-surface | `kind` for records and clipboard; `surface` for surface identity |
| Delivery disposition | `data.disposition` in ref actions, press, pointer, drag, clipboard writes, app and window mutations, and notification mutations, including launch, focus-window, open-system-surface, and both dismiss commands; under `data.after_action` after --wait-for | `disposition` with `delivery`, `retry`; no rename |
| Display and image scale | `scale` in list-displays; `scale_factor`, `format` in screenshot; clipboard-get image has neither | `scale_factor`; clipboard image also has `format` |
| OS, version, architecture | `os` in version; `platform` in status; crate `data.version` in both versus envelope `version: "2.7"`; architecture `target` in version versus operation-report `target` object | `os`; architecture `arch`; version and operation-target names stay distinct by scope |
| Tracing state | `tracing` boolean in status; `trace` as `on` or `off` in session start and list | No rename: writer attachment and session setting are different concepts |
