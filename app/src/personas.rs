//! Sandford NWA — the team-of-teams council.
//!
//! Personas live in `~/.yarp/personas.json`. Edit the file to assemble new
//! squads or rewrite the briefing each persona delivers. `/agent` invokes the
//! default team's council; each persona contributes their angle and the lead
//! delivers the verdict.
//!
//! Structure: a roster has many teams; each team has many personas; one team
//! is marked default. The default team is convened when `/agent` fires with
//! a prompt.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub badge: String,
    pub name: String,
    pub role: String,
    pub voice: String,
    #[serde(default)]
    pub lead: bool,
    /// If set, this persona is backed by a CLI agent binary on disk
    /// (path or just the name). Surfaced in the council framing so the
    /// LLM knows the team includes a real local agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub members: Vec<Persona>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roster {
    pub default_team: String,
    pub teams: Vec<Team>,
}

impl Roster {
    pub fn default_sandford() -> Self {
        let mut members = vec![
            Persona {
                badge: "Sgt".into(),
                name: "Nicholas Angel".into(),
                role: "Lead investigator. Methodical, by-the-book, allergic to shortcuts.".into(),
                voice: "Cuts through fluff, demands evidence, summarises the case at the end.".into(),
                lead: true,
                binary: None,
            },
            Persona {
                badge: "PC".into(),
                name: "Danny Butterman".into(),
                role: "Eager partner. Thinks in action-movie analogies.".into(),
                voice: "Excitable, asks the obvious-but-useful question, pushes for the bold play.".into(),
                lead: false,
                binary: None,
            },
            Persona {
                badge: "PC".into(),
                name: "Doris Thatcher".into(),
                role: "Sharp-eyed for the human angle.".into(),
                voice: "Plain-spoken, calls out what the others missed, names the trade-off.".into(),
                lead: false,
                binary: None,
            },
            Persona {
                badge: "Insp".into(),
                name: "Frank Butterman".into(),
                role: "Strategic devil's advocate.".into(),
                voice: "Probes the second-order consequences and the failure mode nobody wants to discuss.".into(),
                lead: false,
                binary: None,
            },
        ];
        members.extend(detect_cli_personas());
        Self {
            default_team: "Sandford NWA".into(),
            teams: vec![Team {
                name: "Sandford NWA".into(),
                members,
            }],
        }
    }

    pub fn load() -> Option<Self> {
        let path = path()?;
        let bytes = std::fs::read(&path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write_default_if_missing() {
        let Some(path) = path() else { return };
        if path.exists() {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&Self::default_sandford()) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn default_team(&self) -> Option<&Team> {
        self.teams
            .iter()
            .find(|t| t.name == self.default_team)
            .or_else(|| self.teams.first())
    }
}

fn path() -> Option<std::path::PathBuf> {
    Some(yarp_core::paths::yarp_home_config_dir()?.join("personas.json"))
}

/// Probe for installed CLI coding agents and return them as personas. Each
/// detected binary becomes a constable on the council with its install path
/// recorded. Detection runs `zsh -ilc 'command -v <bin>'` so it sees the
/// user's shell PATH (Homebrew, npm-global, cargo, etc.) — not the GUI app's
/// minimal environment.
fn detect_cli_personas() -> Vec<Persona> {
    const KNOWN: &[(&str, &str, &str, &str)] = &[
        // (binary, badge_label, role, voice)
        (
            "claude",
            "Anthropic",
            "Anthropic Claude CLI on this machine.",
            "Careful and structured; prefers to read first, asks before destructive moves, lays out reasoning.",
        ),
        (
            "codex",
            "OpenAI",
            "OpenAI Codex CLI on this machine.",
            "Code-first and terse; jumps straight to a diff, names trade-offs only when forced.",
        ),
        (
            "gemini",
            "Google",
            "Google Gemini CLI on this machine.",
            "Synthesises broadly; surfaces alternatives the others didn't consider.",
        ),
        (
            "aider",
            "Aider",
            "Aider CLI on this machine.",
            "Git-aware and edit-focused; opinionated about diffs and commit hygiene.",
        ),
        (
            "cursor-agent",
            "Cursor",
            "Cursor agent CLI on this machine.",
            "IDE-native; comfortable with multi-file refactors and fast iteration.",
        ),
    ];
    let mut found = Vec::new();
    for (bin, badge_label, role, voice) in KNOWN {
        if let Some(path) = which_via_login_shell(bin) {
            found.push(Persona {
                badge: format!("PC ({badge_label})"),
                name: (*bin).to_string(),
                role: format!("{role} Lives at {}.", path),
                voice: (*voice).to_string(),
                lead: false,
                binary: Some(path),
            });
        }
    }
    found
}

fn which_via_login_shell(bin: &str) -> Option<String> {
    use std::process::Command;
    // -i (interactive) loads .zshrc, -l (login) loads .zprofile. `command -v`
    // is the POSIX-portable way to resolve a name to its path; safer than
    // `which` which may itself not be on PATH inside a sandboxed shell.
    // .zshrc plugins (session restore, autosuggestions) may print to stdout
    // before our command runs, so take only the last non-empty line.
    let cmd = format!("command -v {bin}");
    let out = Command::new("zsh").args(["-ilc", &cmd]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let last = s.lines().rev().map(|l| l.trim()).find(|l| !l.is_empty())?;
    if last.starts_with('/') {
        Some(last.to_owned())
    } else {
        None
    }
}

/// Wrap a user prompt with council framing. Returns None if no roster is
/// configured or the default team has no members; caller falls back to the
/// raw prompt.
pub fn convene_council(prompt: &str) -> Option<String> {
    let roster = Roster::load()?;
    let team = roster.default_team()?;
    if team.members.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str("[Sandford NWA council convened — ");
    let lead = team.members.iter().find(|p| p.lead).or_else(|| team.members.first())?;
    out.push_str(&format!("{} {} presides", lead.badge, lead.name));
    let supporting: Vec<_> = team
        .members
        .iter()
        .filter(|p| !p.lead || p.name != lead.name)
        .collect();
    if !supporting.is_empty() {
        out.push_str("; ");
        let names: Vec<_> = supporting
            .iter()
            .map(|p| format!("{} {}", p.badge, p.name))
            .collect();
        out.push_str(&names.join(", "));
        out.push_str(" weigh in");
    }
    out.push_str(".]\n\n");
    out.push_str("Roster briefing:\n");
    for p in &team.members {
        let suffix = match &p.binary {
            Some(path) => format!(" (CLI agent at {path})"),
            None => String::new(),
        };
        out.push_str(&format!(
            "- **{} {}**{suffix} — {} {}\n",
            p.badge, p.name, p.role, p.voice
        ));
    }
    out.push_str("\nFormat your reply as: each persona contributes a labelled section (e.g. `**Sgt Angel:**`) with their angle on the case below. Personas marked as CLI agents speak with the voice of that local tool — give their take in the style that tool would respond. The lead delivers the final verdict last under `**Verdict:**`. Keep each take tight; no waffle.\n\n");
    out.push_str("Case file:\n");
    out.push_str(prompt);
    Some(out)
}
