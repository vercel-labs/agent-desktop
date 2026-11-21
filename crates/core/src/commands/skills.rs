use crate::AppError;
use serde_json::{Value, json};

const SKILL_DESKTOP_MAIN: &str = include_str!("../../../../skills/agent-desktop/SKILL.md");
const SKILL_MACOS_MAIN: &str = include_str!("../../../../skills/agent-desktop-macos/SKILL.md");
const SKILL_WINDOWS_MAIN: &str = include_str!("../../../../skills/agent-desktop-windows/SKILL.md");
const SKILL_LINUX_MAIN: &str = include_str!("../../../../skills/agent-desktop-linux/SKILL.md");

const SKILL_JEV_MAIN: &str = include_str!("../../../../skills/jev-desktop/SKILL.md");

const SKILL_FFI_MAIN: &str = include_str!("../../../../skills/agent-desktop-ffi/SKILL.md");
const SKILL_FFI_REF_BUILD: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/build-and-link.md");
const SKILL_FFI_REF_ERRORS: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/error-handling.md");
const SKILL_FFI_REF_OWNERSHIP: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/ownership.md");
const SKILL_FFI_REF_THREADING: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/threading.md");

struct SkillRef {
    rel_path: &'static str,
    body: &'static str,
}

struct Skill {
    canonical: &'static str,
    aliases: &'static [&'static str],
    summary: &'static str,
    platform: Option<&'static str>,
    main: &'static str,
    refs: &'static [SkillRef],
}

const PLATFORM_ALIAS: &str = "platform";

const CURRENT_PLATFORM: Option<&str> = if cfg!(target_os = "macos") {
    Some("macos")
} else if cfg!(target_os = "windows") {
    Some("windows")
} else if cfg!(target_os = "linux") {
    Some("linux")
} else {
    None
};

const SKILL_FFI_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/build-and-link.md",
        body: SKILL_FFI_REF_BUILD,
    },
    SkillRef {
        rel_path: "references/error-handling.md",
        body: SKILL_FFI_REF_ERRORS,
    },
    SkillRef {
        rel_path: "references/ownership.md",
        body: SKILL_FFI_REF_OWNERSHIP,
    },
    SkillRef {
        rel_path: "references/threading.md",
        body: SKILL_FFI_REF_THREADING,
    },
];

const SKILLS: &[Skill] = &[
    Skill {
        canonical: "agent-desktop",
        aliases: &["desktop", "agent-desktop"],
        summary: "Start here: one headless observe, act, verify workflow, with ref, retry, and unsaved-work safety rules.",
        platform: None,
        main: SKILL_DESKTOP_MAIN,
        refs: &[],
    },
    Skill {
        canonical: "agent-desktop-macos",
        aliases: &["macos", "agent-desktop-macos"],
        summary: "macOS setup and troubleshooting: permissions, AX identity, application-owned surfaces, and headless refusals.",
        platform: Some("macos"),
        main: SKILL_MACOS_MAIN,
        refs: &[],
    },
    Skill {
        canonical: "agent-desktop-windows",
        aliases: &["windows", "agent-desktop-windows"],
        summary: "Windows setup and troubleshooting: PowerShell refs, interactive sessions, UIPI, foreground-safe routes, and verification.",
        platform: Some("windows"),
        main: SKILL_WINDOWS_MAIN,
        refs: &[],
    },
    Skill {
        canonical: "agent-desktop-linux",
        aliases: &["linux", "agent-desktop-linux"],
        summary: "Linux setup and troubleshooting: AT-SPI bus, toolkit identity, X11 versus Wayland, and capture consent.",
        platform: Some("linux"),
        main: SKILL_LINUX_MAIN,
        refs: &[],
    },
    Skill {
        canonical: "agent-desktop-ffi",
        aliases: &["ffi", "agent-desktop-ffi"],
        summary: "Embedding agent-desktop in another process via the C ABI. Build/link, error propagation, handle ownership, threading rules.",
        platform: None,
        main: SKILL_FFI_MAIN,
        refs: SKILL_FFI_REFS,
    },
    Skill {
        canonical: "jev-desktop",
        aliases: &["jev", "jev-desktop"],
        summary: "Driving a desktop app from a plain-language goal without reading the accessibility tree. run.mjs takes a whole goal, act.mjs takes one step.",
        platform: None,
        main: SKILL_JEV_MAIN,
        refs: &[],
    },
];

pub struct GetArgs {
    pub name: String,
    pub full: bool,
    pub reference: Option<String>,
}

pub fn list() -> Result<Value, AppError> {
    let entries: Vec<Value> = SKILLS
        .iter()
        .map(|s| {
            let mut entry = json!({
                "name": s.canonical,
                "aliases": s.aliases,
                "summary": s.summary,
                "references": s.refs.iter().map(|r| r.rel_path).collect::<Vec<_>>(),
            });
            if let Some(platform) = s.platform {
                entry["platform"] = json!(platform);
                entry["current"] = json!(CURRENT_PLATFORM == Some(platform));
            }
            entry
        })
        .collect();
    Ok(json!({ "skills": entries }))
}

pub fn get(args: GetArgs) -> Result<Value, AppError> {
    let skill = find_skill(&args.name)?;

    if let Some(rel) = args.reference {
        let refs = skill.refs;
        let r = refs
            .iter()
            .find(|r| matches_ref(r.rel_path, &rel))
            .ok_or_else(|| {
                let available: Vec<&str> = refs.iter().map(|r| r.rel_path).collect();
                AppError::invalid_input(format!(
                    "Unknown reference '{rel}' for skill '{}'. Available: {}",
                    skill.canonical,
                    available.join(", ")
                ))
            })?;
        return Ok(json!({
            "skill": skill.canonical,
            "reference": r.rel_path,
            "content": r.body,
        }));
    }

    let content = if args.full {
        render_full(skill)
    } else {
        skill.main.to_string()
    };

    Ok(json!({
        "skill": skill.canonical,
        "full": args.full,
        "content": content,
    }))
}

pub fn path() -> Result<Value, AppError> {
    Ok(json!({
        "location": "embedded",
        "note": "Skills are compiled into this binary and are always version-matched. Run `agent-desktop skills get <name>` to print a skill, or redirect into a file to extract a copy.",
        "available": SKILLS.iter().map(|s| s.canonical).collect::<Vec<_>>(),
    }))
}

fn find_skill(name: &str) -> Result<&'static Skill, AppError> {
    let needle = name.trim();
    if needle.eq_ignore_ascii_case(PLATFORM_ALIAS) {
        return platform_skill();
    }
    SKILLS
        .iter()
        .find(|s| s.aliases.iter().any(|a| a.eq_ignore_ascii_case(needle)))
        .ok_or_else(|| {
            let known: Vec<&str> = SKILLS
                .iter()
                .flat_map(|s| s.aliases.iter().copied())
                .collect();
            AppError::invalid_input(format!(
                "Unknown skill '{name}'. Known: {}",
                known.join(", ")
            ))
        })
}

fn platform_skill() -> Result<&'static Skill, AppError> {
    SKILLS
        .iter()
        .find(|s| s.platform.is_some() && s.platform == CURRENT_PLATFORM)
        .ok_or_else(|| {
            AppError::invalid_input_with_suggestion(
                "This build's operating system has no platform skill yet",
                "Use `agent-desktop skills get desktop`; it covers the loop on every OS",
            )
        })
}

fn matches_ref(rel_path: &str, query: &str) -> bool {
    if rel_path.eq_ignore_ascii_case(query) {
        return true;
    }
    let file = rel_path.rsplit('/').next().unwrap_or(rel_path);
    let stem = file.strip_suffix(".md").unwrap_or(file);
    file.eq_ignore_ascii_case(query) || stem.eq_ignore_ascii_case(query)
}

fn render_full(skill: &Skill) -> String {
    let refs = skill.refs;
    let mut out = String::with_capacity(
        skill.main.len() + refs.iter().map(|r| r.body.len() + 64).sum::<usize>(),
    );
    out.push_str(skill.main);
    for r in refs {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n--- ");
        out.push_str(r.rel_path);
        out.push_str(" ---\n\n");
        out.push_str(r.body);
    }
    out
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "skills_workflow_tests.rs"]
mod workflow_tests;
