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
        // codex refuses to run outside a trusted git repo unless told to skip
        // the check. The council can be invoked from anywhere (e.g. ~), so
        // pass the flag unconditionally.
        "codex" => &["exec", "--skip-git-repo-check"],
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

/// Like `shell_quote_one`, but skips the quotes for inputs that contain only
/// "safe" characters — alphanumerics and the punctuation set used in flags
/// and paths (`-`, `_`, `.`, `/`, `=`, `:`, `,`, `+`, `@`, `%`). Result is
/// still safe to feed to `try_execute_command` because every character that
/// could trigger word-splitting, expansion, redirection, or substitution is
/// quoted. Use this when building command strings shown to the user — it
/// reads dramatically cleaner than `'-p' 'exec' '--flag'`.
pub(crate) fn shell_quote_smart(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let safe = s.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '/' | '=' | ':' | ',' | '+' | '@' | '%')
    });
    if safe {
        s.to_string()
    } else {
        shell_quote_one(s)
    }
}

/// ANSI 256-color escape (bold + foreground) for a persona's header line,
/// keyed off the persona's display name. Brand-aligned where it makes
/// sense — Anthropic orange, OpenAI green, Google blue — and gold for the
/// team lead so the verdict block stands out. Unknown names fall back to
/// plain bold so the header still pops.
pub(crate) fn persona_header_color(name: &str) -> &'static str {
    match name {
        "claude" => "\\033[1;38;5;208m",
        "codex" => "\\033[1;38;5;35m",
        "gemini" => "\\033[1;38;5;39m",
        "Nicholas Angel" => "\\033[1;38;5;220m",
        _ => "\\033[1m",
    }
}

/// Build the shell command for one persona's CLI invocation in the council
/// chain. The generic shape is `<binary> <plain_args> '<prompt>' | tee
/// <take_file>` — stdout is mirrored to the take file so the synth pass can
/// re-read every persona's answer. Some CLIs print so much banner /
/// narration / framing noise that a plain `tee` makes the council block
/// unreadable AND poisons the synth's input; for those we tailor the
/// invocation so the take file holds a clean final answer and the visible
/// block stays focused on the answer.
pub(crate) fn build_persona_cmd(
    basename: &str,
    program: &str,
    prompt: &str,
    take_file: &str,
) -> String {
    let prog = shell_quote_smart(program);
    let take = shell_quote_smart(take_file);
    let prompt_q = shell_quote_one(prompt);
    match basename {
        // codex prints a verbose banner (workdir/model/provider/approval/
        // sandbox/reasoning effort/session id), an "exec" narration of the
        // shell calls it makes, an `ERROR codex_core::session: failed to
        // record rollout items` line, and a duplicated final answer.
        // `--output-last-message` writes ONLY the final agent message to a
        // file; `--ephemeral` skips session persistence (suppresses the
        // rollout-items error). Hide stdout entirely (it's pure noise) and
        // `cat` the take file at the end so the block displays the clean
        // answer once codex finishes.
        "codex" => format!(
            "{prog} exec --skip-git-repo-check --ephemeral --output-last-message {take} {prompt_q} >/dev/null 2>&1; cat {take}; echo",
        ),
        // Gemini prints `Loaded cached credentials.` to STDERR as a preamble
        // before the real answer. Merge stderr into stdout (so the line goes
        // through the same pipe as the answer), then filter that one line
        // before tee so the take file and the visible block stay focused on
        // the agent's response.
        "gemini" => format!(
            "{prog} -p {prompt_q} 2>&1 | grep -vF 'Loaded cached credentials.' | tee {take}",
        ),
        _ => {
            let args = plain_args_for(basename)
                .iter()
                .map(|a| shell_quote_smart(a))
                .collect::<Vec<_>>()
                .join(" ");
            if args.is_empty() {
                format!("{prog} {prompt_q} | tee {take}")
            } else {
                format!("{prog} {args} {prompt_q} | tee {take}")
            }
        }
    }
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

