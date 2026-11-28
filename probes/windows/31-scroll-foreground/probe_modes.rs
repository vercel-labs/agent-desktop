//! The probe modes: `tops`, `find`, `run` (the Part A matrix), `act` (one
//! pattern call) and `cli` (one shipped command), each on the product client.

use crate::{Args, digest_of};
use crate::calls::{self, Outcome, Selector};
use crate::hook::Hook;
use crate::run::{Arrangement, Stage};
use crate::state;
use serde_json::{Value, json};
use uiautomation::patterns::UIScrollPattern;
use uiautomation::types::Handle;
use uiautomation::{UIAutomation, UIElement};
use windows::Win32::UI::Accessibility::{IUIAutomation, IUIAutomation2};
use windows::core::Interface;

const A2_PERCENTS: [f64; 5] = [20.0, 60.0, 35.0, 80.0, 50.0];

/// The product client with `IUIAutomation2::AutoSetFocus` set as asked.
/// `off` is what the product ships. Any other value sets the UI Automation
/// default, which makes control pattern methods set focus to the target
/// element first.
fn client(args: &Args) -> Result<UIAutomation, String> {
    agent_desktop_windows::ensure_owned_process_mta_and_dpi()
        .map_err(|error| format!("bootstrap failed: {}", error.message))?;
    let automation = agent_desktop_windows::tree::automation::automation_client()
        .map_err(|error| format!("client failed: {}", error.message))?;
    let enabled = args.get("auto-set-focus") != Some("off");
    let raw: &IUIAutomation = automation.as_ref();
    let extended: IUIAutomation2 = raw.cast().map_err(|error| format!("IUIAutomation2: {error}"))?;
    unsafe { extended.SetAutoSetFocus(enabled) }.map_err(|error| format!("SetAutoSetFocus: {error}"))?;
    Ok(automation)
}

fn auto_set_focus(automation: &UIAutomation) -> Option<bool> {
    let raw: &IUIAutomation = automation.as_ref();
    let extended: IUIAutomation2 = raw.cast().ok()?;
    unsafe { extended.AutoSetFocus() }.ok().map(|value| value.as_bool())
}

fn selector(args: &Args) -> Selector {
    Selector {
        automation_id: args.get("aid").map(String::from),
        class: args.get("cls").map(String::from),
        control_type: args.get("ct").map(String::from),
        nth: args.count("nth", 0),
    }
}

pub fn tops() -> Result<Value, String> {
    let rows: Vec<Value> = state::top_level_windows()
        .into_iter()
        .filter(|handle| state::is_visible(*handle))
        .map(|handle| {
            json!({
                "hwnd": handle,
                "pid": state::window_pid(handle),
                "class": state::class_of(handle),
                "title": state::title_of(handle),
                "iconic": state::is_iconic(handle),
            })
        })
        .collect();
    Ok(json!(rows))
}

pub fn find(args: &Args) -> Result<Value, String> {
    let automation = client(args)?;
    let target = args.handle("t")?;
    crate::run::place_for_discovery(target);
    let root = automation
        .element_from_handle(Handle::from(args.handle("root").unwrap_or(target)))
        .map_err(|error| format!("root failed: {error}"))?;
    let (scrollables, source) = calls::find_scrollables(&automation, &root);
    Ok(json!({
        "target": state::describe_window(target),
        "tree_census": calls::census(&automation, &root),
        "source": source,
        "vertically_scrollable": scrollables.iter().map(calls::describe).collect::<Vec<_>>(),
        "first_focusable": calls::first_focusable(&automation, &root).as_ref().map(calls::describe),
        "first_item_container": calls::first_item_container(&automation, &root).as_ref().map(calls::describe),
    }))
}

struct Scene {
    automation: UIAutomation,
    f: isize,
    t: isize,
    root: UIElement,
    scroll_element: Option<UIElement>,
    scrollables: Vec<Value>,
    source: &'static str,
}

fn scene(args: &Args) -> Result<Scene, String> {
    let automation = client(args)?;
    let f = args.handle("f")?;
    let t = args.handle("t")?;
    if !state::is_window(f) || !state::is_window(t) {
        return Err("F or T is not a live window".into());
    }
    crate::run::place_for_discovery(t);
    let root_handle = args.handle("root").unwrap_or(t);
    let root = automation
        .element_from_handle(Handle::from(root_handle))
        .map_err(|error| format!("root failed: {error}"))?;
    let (scrollables, source) = calls::find_scrollables(&automation, &root);
    let scroll_element = calls::pick(&scrollables, &selector(args));
    Ok(Scene {
        automation,
        f,
        t,
        root,
        scroll_element,
        scrollables: scrollables.iter().take(12).map(calls::describe).collect(),
        source,
    })
}

fn stage<'a>(scene: &'a Scene, pattern: Option<UIScrollPattern>) -> Stage<'a> {
    let f_focus = scene
        .automation
        .element_from_handle(Handle::from(scene.f))
        .ok()
        .map(|root| calls::first_focusable(&scene.automation, &root).unwrap_or(root));
    Stage {
        automation: &scene.automation,
        f: scene.f,
        t: scene.t,
        f_focus,
        scroll: pattern,
        arrangement: Arrangement::Beside,
    }
}

fn with_detail(mut trial: Value, detail: Value) -> Value {
    if let Some(map) = trial.as_object_mut() {
        map.insert("detail".into(), detail);
    }
    trial
}

pub fn matrix(args: &Args) -> Result<Value, String> {
    let scene = scene(args)?;
    let hook = Hook::start()?;
    let Some(scroll_element) = scene.scroll_element.clone() else {
        hook.stop();
        return Ok(json!({
            "label": args.get("label"),
            "target": state::describe_window(scene.t),
            "no_vertically_scrollable_element": true,
            "discovery": scene.source,
            "candidates": scene.scrollables,
        }));
    };
    let pattern = scroll_element
        .get_pattern::<UIScrollPattern>()
        .map_err(|error| format!("ScrollPattern failed: {error}"))?;
    let focus_target = if scroll_element.is_keyboard_focusable().unwrap_or(false) {
        scroll_element.clone()
    } else {
        calls::first_focusable(&scene.automation, &scene.root).unwrap_or_else(|| scroll_element.clone())
    };
    let container_element = if calls::item_container(&scroll_element).is_some() {
        Some(scroll_element.clone())
    } else {
        calls::first_item_container(&scene.automation, &scene.root)
    };
    let container = container_element.as_ref().and_then(calls::item_container);
    let names: Vec<String> = args
        .get("item-names")
        .map(|text| text.split('|').map(String::from).collect())
        .unwrap_or_default();
    let trials = args.count("trials", 5);
    let arrangements: Vec<Arrangement> = args
        .get("arrangements")
        .unwrap_or("beside,covered,minimized")
        .split(',')
        .filter_map(Arrangement::parse)
        .collect();
    let element_json = calls::describe(&scroll_element);
    let focus_json = calls::describe(&focus_target);
    let container_json = container_element.as_ref().map(calls::describe);
    let mut stage = stage(&scene, Some(pattern.clone()));
    let mut rows: Vec<Value> = Vec::new();
    let mut entered: Vec<Value> = Vec::new();
    for arrangement in arrangements {
        entered.push(stage.enter(arrangement));
        for index in 1..=2 {
            rows.push(stage.trial("positive_control_SetFocus", index, &mut || {
                calls::set_focus(&focus_target)
            }));
        }
        for index in 1..=trials {
            rows.push(stage.trial("negative_control_CurrentName", index, &mut || {
                calls::read_name(&scroll_element)
            }));
        }
        for index in 1..=trials {
            let toward_end = pattern.get_vertical_scroll_percent().map_or(true, |percent| percent < 95.0);
            let row = stage.trial("A1_Scroll", index, &mut || calls::scroll_small(&pattern, toward_end));
            let amount = if toward_end { "SmallIncrement" } else { "SmallDecrement" };
            rows.push(with_detail(row, json!({ "vertical_amount": amount })));
        }
        for index in 1..=trials {
            let mut wanted = A2_PERCENTS[(index - 1) % A2_PERCENTS.len()];
            if pattern.get_vertical_scroll_percent().is_ok_and(|now| (now - wanted).abs() < 1.0) {
                wanted += 7.0;
            }
            let row = stage.trial("A2_SetScrollPercent", index, &mut || calls::set_percent(&pattern, wanted));
            rows.push(with_detail(row, json!({ "requested_vertical_percent": wanted })));
        }
        let walker = scene.automation.get_raw_view_walker().ok();
        for index in 1..=trials {
            let last = index % 2 == 1;
            let child = walker.as_ref().and_then(|walker| {
                if last { walker.get_last_child(&scroll_element).ok() } else { walker.get_first_child(&scroll_element).ok() }
            });
            let detail = json!({ "item": if last { "last raw child of the scroll element" } else { "first raw child of the scroll element" } });
            let Some(item_pattern) = child.as_ref().and_then(calls::scroll_item) else {
                rows.push(json!({ "arrangement": arrangement.name(), "call": "A5_ScrollIntoView", "trial": index,
                    "skipped": "that child offers no ScrollItemPattern", "detail": detail }));
                continue;
            };
            let row = stage.trial("A5_ScrollIntoView", index, &mut || calls::scroll_into_view(&item_pattern));
            rows.push(with_detail(row, detail));
        }
        let Some(container) = container.as_ref() else {
            rows.push(json!({ "arrangement": arrangement.name(), "call": "A3_FindItemByProperty",
                "skipped": "no element under the target offers ItemContainerPattern" }));
            continue;
        };
        if names.is_empty() {
            rows.push(json!({ "arrangement": arrangement.name(), "call": "A3_FindItemByProperty",
                "skipped": "no item names were given" }));
            continue;
        }
        for index in 1..=trials {
            let name = &names[(index - 1) % names.len()];
            let row = stage.trial("A3_FindItemByProperty", index, &mut || calls::find_item(container, name).0);
            rows.push(with_detail(row, json!({ "name_len": name.len(), "name_digest": digest_of(name) })));
        }
        for index in 1..=trials {
            let name = &names[(index - 1) % names.len()];
            let detail = json!({ "name_len": name.len(), "name_digest": digest_of(name) });
            let (lookup, item) = calls::find_item(container, name);
            let Some(virtualized) = item.as_ref().and_then(calls::virtualized) else {
                rows.push(json!({ "arrangement": arrangement.name(), "call": "A4_Realize", "trial": index,
                    "skipped": if item.is_some() { "the returned item offers no VirtualizedItemPattern" } else { "the lookup returned no item" },
                    "lookup": lookup.to_json(), "detail": detail }));
                continue;
            };
            let row = stage.trial("A4_Realize", index, &mut || calls::realize(&virtualized));
            rows.push(with_detail(row, detail));
        }
    }
    stage.leave();
    hook.stop();
    Ok(json!({
        "label": args.get("label"),
        "session": args.get("session"),
        "client": "uia3-com CUIAutomation8 through agent_desktop_windows::tree::automation::automation_client",
        "auto_set_focus": auto_set_focus(&scene.automation),
        "f": state::describe_window(scene.f),
        "target": state::describe_window(scene.t),
        "discovery_root": args.handle("root").ok().map(state::describe_window),
        "discovery": scene.source,
        "candidates": scene.scrollables,
        "element": element_json,
        "positive_control_element": focus_json,
        "item_container_element": container_json,
        "arrangements": entered,
        "trials_per_cell": trials,
        "rows": rows,
    }))
}

/// `act`: one named pattern call on one element, bracketed like a scroll trial.
pub fn act(args: &Args) -> Result<Value, String> {
    let scene = scene(args)?;
    let hook = Hook::start()?;
    let element = calls::find_by(&scene.automation, &scene.root, args.get("aid"), args.get("name"))
        .ok_or("no element matches --aid / --name")?;
    let call = args.get("call").ok_or("missing --call")?.to_string();
    let values: Vec<String> = args.get("values").unwrap_or("").split('|').map(String::from).collect();
    let trials = args.count("trials", 5);
    let arrangement = args.get("arrangement").and_then(Arrangement::parse).unwrap_or(Arrangement::Beside);
    let mut stage = stage(&scene, None);
    let entered = stage.enter(arrangement);
    let mut rows: Vec<Value> = Vec::new();
    for index in 1..=trials {
        let value = values[(index - 1) % values.len()].clone();
        let alternate = match (call.as_str(), index % 2) {
            ("expand", 0) => "collapse",
            ("collapse", 0) => "expand",
            (other, _) => other,
        };
        let before = calls::effect_state(&element);
        let row = stage.trial(alternate, index, &mut || calls::pattern_call(&element, alternate, &value));
        let after = calls::effect_state(&element);
        rows.push(with_detail(row, json!({ "effect_before": before, "effect_after": after, "effect_changed": before != after })));
    }
    hook.stop();
    Ok(json!({
        "label": args.get("label"),
        "auto_set_focus": auto_set_focus(&scene.automation),
        "call": call,
        "arrangement": entered,
        "element": calls::describe(&element),
        "rows": rows,
    }))
}

pub fn cli(args: &Args) -> Result<Value, String> {
    let scene = scene(args)?;
    let hook = Hook::start()?;
    let exe = args.get("exe").ok_or("missing --exe")?.to_string();
    let argv: Vec<String> = std::fs::read_to_string(args.get("args-file").ok_or("missing --args-file")?)
        .map_err(|error| format!("args file: {error}"))
        .and_then(|text| {
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|error| format!("args json: {error}"))
        })?;
    let pattern = scene
        .scroll_element
        .as_ref()
        .and_then(|element| element.get_pattern::<UIScrollPattern>().ok());
    let arrangement = args.get("arrangement").and_then(Arrangement::parse).unwrap_or(Arrangement::Beside);
    let mut stage = stage(&scene, pattern);
    let entered = stage.enter(arrangement);
    let mut output: Option<std::process::Output> = None;
    let selection_before = scene.scroll_element.as_ref().and_then(calls::selected_names);
    let row = stage.trial(args.get("label").unwrap_or("cli"), 1, &mut || {
        let started = std::time::Instant::now();
        let result = std::process::Command::new(&exe).args(&argv).output();
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let code = result.as_ref().ok().and_then(|output| output.status.code());
        let note = match &result {
            Ok(_) => format!("exit code {code:?}"),
            Err(error) => format!("spawn failed: {error}"),
        };
        output = result.ok();
        Outcome { ok: code == Some(0), hresult: None, sentinel: code, note: Some(note), ms }
    });
    hook.stop();
    let stdout = output.as_ref().map(|output| String::from_utf8_lossy(&output.stdout).to_string()).unwrap_or_default();
    let envelope: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|_| json!({ "unparsed_stdout_len": stdout.len() }));
    Ok(json!({
        "label": args.get("label"),
        "auto_set_focus_of_probe_client": auto_set_focus(&scene.automation),
        "argv": argv,
        "arrangement": entered,
        "element": scene.scroll_element.as_ref().map(calls::describe),
        "selection_before": selection_before,
        "selection_after": scene.scroll_element.as_ref().and_then(calls::selected_names),
        "envelope": envelope,
        "row": row,
    }))
}
