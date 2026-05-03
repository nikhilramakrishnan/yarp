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

use std::os::unix::fs::PermissionsExt as _;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Map a binary basename to the one-shot invocation that takes the prompt as
/// its trailing argument. e.g. claude → `claude -p`, codex → `codex exec`.
/// Anything unknown is invoked bare (the prompt as the only argument).
fn oneshot_args_for(binary_path: &str) -> Vec<&'static str> {
    let basename = std::path::Path::new(binary_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match basename {
        "claude" => vec!["-p"],
        "codex" => vec!["exec"],
        "gemini" => vec!["-p"],
        "aider" => vec!["--message"],
        "cursor-agent" => vec!["--print"],
        _ => vec![],
    }
}

/// Build a self-contained bash script that fans out the prompt across every
/// CLI persona in the default team, then runs one final synthesis pass
/// through the chosen lead. Returns the command string to execute (e.g.
/// `bash /tmp/yarp-council-<uuid>.sh`), plus the temp files written so the
/// caller can clean them up if it wants. Returns None if there's no roster
/// or no CLI personas to dispatch to.
pub fn build_council_command(prompt: &str) -> Option<String> {
    let roster = Roster::load()?;
    let team = roster.default_team()?;
    let cli_personas: Vec<&Persona> = team.members.iter().filter(|p| p.binary.is_some()).collect();
    if cli_personas.is_empty() {
        return None;
    }
    let synth = synthesiser(team);

    let id = Uuid::new_v4();
    let tmp = std::env::temp_dir();
    let prompt_path = tmp.join(format!("yarp-council-{id}-prompt.txt"));
    let script_path = tmp.join(format!("yarp-council-{id}.sh"));
    std::fs::write(&prompt_path, prompt).ok()?;

    let mut script = String::new();
    script.push_str("#!/bin/bash\n");
    script.push_str("set -uo pipefail\n");
    script.push_str(&format!(
        "PROMPT_FILE={}\n",
        shell_escape(&prompt_path.to_string_lossy())
    ));
    script.push_str("PROMPT=\"$(cat \"$PROMPT_FILE\")\"\n");
    script.push_str("WORK=$(mktemp -d)\n");
    // Clean up the temp work dir, the prompt file, and this script itself
    // on exit. Bash already loaded the script into memory; deleting the
    // file mid-execution is safe.
    script.push_str("trap 'rm -rf \"$WORK\" \"$PROMPT_FILE\" \"$0\"' EXIT\n");
    script.push_str("\n");
    script.push_str("printf '🚓 **Sandford NWA convening on:** %s\\n\\n' \"$PROMPT\"\n");

    for (idx, p) in cli_personas.iter().enumerate() {
        let bin = p.binary.as_ref().expect("filter guarantees Some");
        let args = oneshot_args_for(bin);
        let mut cmd = String::from(&shell_escape(bin));
        for a in &args {
            cmd.push(' ');
            cmd.push_str(&shell_escape(a));
        }
        cmd.push(' ');
        cmd.push_str("\"$PROMPT\"");
        let header = format!("### {} {} ({})", p.badge, p.name, bin);
        let out_file = format!("$WORK/take-{idx}.out");
        script.push_str("\n");
        script.push_str(&format!(
            "printf '%s\\n\\n' {}\n",
            shell_escape(&header)
        ));
        // tee so we both stream to the terminal and capture for synthesis.
        script.push_str(&format!("{cmd} 2>&1 | tee {out_file}\n"));
        script.push_str("printf '\\n'\n");
    }

    if let Some(lead) = synth {
        let lead_bin = lead
            .binary
            .as_ref()
            .expect("synthesiser is filtered for binary.is_some()");
        let lead_args = oneshot_args_for(lead_bin);
        let mut lead_cmd = String::from(&shell_escape(lead_bin));
        for a in &lead_args {
            lead_cmd.push(' ');
            lead_cmd.push_str(&shell_escape(a));
        }
        lead_cmd.push(' ');
        lead_cmd.push_str("\"$SYNTH_PROMPT\"");

        script.push_str("\n");
        script.push_str(&format!(
            "printf '### **Verdict** — synthesised by {} {}\\n\\n'\n",
            lead.badge, lead.name
        ));
        script.push_str("SYNTH_PROMPT=\"You are the lead of the Sandford NWA on this case:\n");
        script.push_str("\n");
        script.push_str("$PROMPT\n\n");
        for (idx, p) in cli_personas.iter().enumerate() {
            script.push_str(&format!(
                "{} {} said:\n$(cat $WORK/take-{idx}.out)\n\n",
                p.badge, p.name
            ));
        }
        script.push_str(
            "Identify points of agreement and disagreement, name the trade-off, and deliver a tight final verdict. Cut the fluff.\"\n\n",
        );
        script.push_str(&format!("{lead_cmd}\n"));
    }

    std::fs::write(&script_path, script).ok()?;
    let _ = std::fs::set_permissions(
        &script_path,
        std::fs::Permissions::from_mode(0o755),
    );

    Some(format!("bash {}", shell_escape(&script_path.to_string_lossy())))
}

/// Single-quote a shell argument: wrap in `'...'` and escape any inner `'`
/// as `'\''`. Safe for arbitrary content.
fn shell_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

