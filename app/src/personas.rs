//! Sandford NWA — the team-of-teams council.
//!
//! Personas live in `~/.yarp/personas.json`. Edit the file to assemble new
//! squads or change which CLI each persona is backed by. `/agent` invokes
//! the default team's council; each CLI persona runs the prompt and the
//! lead delivers a synthesised verdict.
//!
//! `role` and `voice` are display metadata for the Council settings page —
//! they're not injected into the prompt sent to each CLI.
//!
//! Structure: a roster has many teams; each team has many personas; one
//! team is marked default. The default team is convened when `/agent`
//! fires with a prompt.

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
    use std::path::PathBuf;
    use std::process::Command;
    // Preferred install location for CLI agents on this user's machines is
    // ~/.local/bin (symlinks to versioned dirs under .codex, .claude, etc.).
    // Check that first so we don't pick up stale Homebrew copies.
    if let Some(home) = std::env::var_os("HOME") {
        let candidate = PathBuf::from(home).join(".local").join("bin").join(bin);
        if candidate.exists() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    // Fall back to login-shell PATH resolution. -i (interactive) loads .zshrc,
    // -l (login) loads .zprofile. `command -v` is POSIX-portable. .zshrc
    // plugins (session restore, autosuggestions) may print to stdout before
    // our command runs, so take only the last non-empty line.
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

/// Pick the synthesiser for a verdict pass: prefer a lead CLI persona; else
/// the first CLI persona in the team.
pub(crate) fn synthesiser(team: &Team) -> Option<&Persona> {
    team.members
        .iter()
        .find(|p| p.lead && p.binary.is_some())
        .or_else(|| team.members.iter().find(|p| p.binary.is_some()))
}

/// One CLI invocation ready to spawn: persona, the binary path, and the
/// argv prefix that puts the CLI in **streaming JSON mode**. The caller
/// appends the user prompt as the trailing positional argument.
pub(crate) struct CliInvocation<'a> {
    pub persona: &'a Persona,
    pub program: String,
    pub streaming_args: Vec<String>,
    pub binary_basename: String,
}

/// Iterate the team's CLI-backed personas and yield one invocation each, in
/// streaming JSON mode (so the UI can render Thinking vs Output phases
/// distinctly). Personas without a binary are skipped — the council view is
/// only useful when at least one CLI is present.
pub(crate) fn cli_invocations(team: &Team) -> Vec<CliInvocation<'_>> {
    team.members
        .iter()
        .filter_map(|p| {
            let binary = p.binary.as_deref()?;
            let basename = std::path::Path::new(binary)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned();
            let args = streaming_args_for(&basename)
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            Some(CliInvocation {
                persona: p,
                program: binary.to_owned(),
                streaming_args: args,
                binary_basename: basename,
            })
        })
        .collect()
}

/// Argv prefix that puts each known CLI in newline-delimited JSON streaming
/// mode. Trailing arg should be the prompt.
/// Argv prefix for plain (non-JSON-streaming) one-shot output. The new
/// shell-command council chain uses plain mode so each persona's output
/// renders as a normal native terminal block. Mirrors `streaming_args_for`
/// shape but drops the structured-event flags.
pub(crate) fn plain_args_for(basename: &str) -> &'static [&'static str] {
    match basename {
        "claude" => &["-p"],
        "codex" => &["exec"],
        "gemini" => &["-p"],
        "aider" => &["--message"],
        "cursor-agent" => &["--print"],
        _ => &[],
    }
}

/// Single-quote shell quoting for one argument: wraps `s` in `'…'` and
/// escapes any embedded single-quote as `'\''`. Safe for arbitrary content
/// and the right primitive for building shell command strings to feed to
/// `try_execute_command`.
pub(crate) fn shell_quote_one(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

pub(crate) fn streaming_args_for(basename: &str) -> &'static [&'static str] {
    match basename {
        "claude" => &["-p", "--output-format", "stream-json", "--verbose"],
        "codex" => &["exec", "--json"],
        // gemini's streaming mode isn't pinned; fall back to one-shot text.
        "gemini" => &["-p"],
        "aider" => &["--message"],
        "cursor-agent" => &["--print"],
        _ => &[],
    }
}

