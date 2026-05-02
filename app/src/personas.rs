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
        Self {
            default_team: "Sandford NWA".into(),
            teams: vec![Team {
                name: "Sandford NWA".into(),
                members: vec![
                    Persona {
                        badge: "Sgt".into(),
                        name: "Nicholas Angel".into(),
                        role: "Lead investigator. Methodical, by-the-book, allergic to shortcuts.".into(),
                        voice: "Cuts through fluff, demands evidence, summarises the case at the end.".into(),
                        lead: true,
                    },
                    Persona {
                        badge: "PC".into(),
                        name: "Danny Butterman".into(),
                        role: "Eager partner. Thinks in action-movie analogies.".into(),
                        voice: "Excitable, asks the obvious-but-useful question, pushes for the bold play.".into(),
                        lead: false,
                    },
                    Persona {
                        badge: "PC".into(),
                        name: "Doris Thatcher".into(),
                        role: "Sharp-eyed for the human angle.".into(),
                        voice: "Plain-spoken, calls out what the others missed, names the trade-off.".into(),
                        lead: false,
                    },
                    Persona {
                        badge: "Insp".into(),
                        name: "Frank Butterman".into(),
                        role: "Strategic devil's advocate.".into(),
                        voice: "Probes the second-order consequences and the failure mode nobody wants to discuss.".into(),
                        lead: false,
                    },
                ],
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
        out.push_str(&format!(
            "- **{} {}** — {} {}\n",
            p.badge, p.name, p.role, p.voice
        ));
    }
    out.push_str("\nFormat your reply as: each persona contributes a labelled section (e.g. `**Sgt Angel:**`) with their angle on the case below. The lead delivers the final verdict last under `**Verdict:**`. Keep each take tight; no waffle.\n\n");
    out.push_str("Case file:\n");
    out.push_str(prompt);
    Some(out)
}
