//! Inter-terminal radio: each running Yarp drops a JSON beacon at
//! `~/.yarp/radio/<pid>.json` so peers can discover it. Stale beacons (pid no
//! longer alive) are pruned on read.
//!
//! Hot Fuzz lexicon: each beacon is an officer's call sign on the channel; the
//! radio dir is the precinct's open frequency.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Beacon {
    pub pid: u32,
    pub started_at_unix: u64,
    pub hostname: String,
    pub app_id: String,
    pub call_sign: String,
    /// Visible window/tab label, used by peer pickers so officers can tell
    /// each other apart at a glance. `None` until the GUI publishes one.
    #[serde(default)]
    pub tab_title: Option<String>,
}

impl Beacon {
    pub fn new(app_id: impl Into<String>, call_sign: impl Into<String>) -> Self {
        Self {
            pid: std::process::id(),
            started_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            hostname: hostname(),
            app_id: app_id.into(),
            call_sign: call_sign.into(),
            tab_title: None,
        }
    }

    /// Builder-style setter for the visible tab label.
    pub fn with_tab_title(mut self, tab_title: impl Into<String>) -> Self {
        self.tab_title = Some(tab_title.into());
        self
    }
}

/// Republish a fresh beacon for the current process — used when the visible
/// tab label changes and we want peers to see the new one. Best-effort.
pub fn update_tab_title(call_sign: impl Into<String>, tab_title: impl Into<String>) {
    let beacon = Beacon::new("dev.yarp.Yarp", call_sign).with_tab_title(tab_title);
    let _ = register(&beacon);
}

/// The call sign this process would broadcast on registration. Mirrors
/// `bin/yarp.rs::default_call_sign` so other modules (status bar, about
/// page) can render "you are X on the air" without re-deriving it.
pub fn self_call_sign() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| format!("Officer-{}", std::process::id()))
}

/// View-friendly summary of a peer on the channel — what a picker UI or
/// `/radio` listing wants to render. Uptime is computed at read time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSummary {
    pub pid: u32,
    pub call_sign: String,
    pub tab_title: Option<String>,
    pub hostname: String,
    pub uptime_secs: u64,
}

impl PeerSummary {
    fn from_beacon(beacon: Beacon, now_unix: u64) -> Self {
        let uptime_secs = now_unix.saturating_sub(beacon.started_at_unix);
        Self {
            pid: beacon.pid,
            call_sign: beacon.call_sign,
            tab_title: beacon.tab_title,
            hostname: beacon.hostname,
            uptime_secs,
        }
    }
}

/// Render the active peer list as a human-readable multi-line string —
/// what a `/radio` slash command, status-bar tooltip, or debug pane wants
/// to dump. Empty roster gets a Sandford-flavored placeholder.
pub fn format_peer_list(peer_list: &[PeerSummary]) -> String {
    if peer_list.is_empty() {
        return "No other officers on the channel — sole patrol.".to_string();
    }
    let mut out = String::new();
    for peer in peer_list {
        let label = peer.tab_title.as_deref().unwrap_or("Unfiled patrol");
        let uptime = format_uptime(peer.uptime_secs);
        out.push_str(&format!(
            "{call_sign} · {label} · pid {pid} · on the air for {uptime}\n",
            call_sign = peer.call_sign,
            pid = peer.pid,
        ));
    }
    out
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h{m:02}m")
    } else if m > 0 {
        format!("{m}m{s:02}s")
    } else {
        format!("{s}s")
    }
}

/// Active peers on the radio, excluding this process. Sorted by call sign
/// for stable picker rendering.
pub fn peers() -> Vec<PeerSummary> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let self_pid = std::process::id();
    let mut out: Vec<PeerSummary> = list_active()
        .into_iter()
        .filter(|b| b.pid != self_pid)
        .map(|b| PeerSummary::from_beacon(b, now))
        .collect();
    out.sort_by(|a, b| a.call_sign.cmp(&b.call_sign).then(a.pid.cmp(&b.pid)));
    out
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn radio_dir() -> Option<PathBuf> {
    yarp_core::paths::yarp_home_radio_dir()
}

fn beacon_path(pid: u32) -> Option<PathBuf> {
    radio_dir().map(|dir| dir.join(format!("{pid}.json")))
}

/// Drop a beacon for this process. Existing beacons for the same pid are
/// overwritten. Failures are logged and swallowed — radio is best-effort.
pub fn register(beacon: &Beacon) -> io::Result<()> {
    let Some(dir) = radio_dir() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no home dir for radio",
        ));
    };
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", beacon.pid));
    let json = serde_json::to_vec_pretty(beacon)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&path, json)
}

/// Remove this process's beacon. Idempotent.
pub fn deregister(pid: u32) {
    if let Some(path) = beacon_path(pid) {
        let _ = fs::remove_file(path);
    }
}

/// Hold this for the lifetime of the process. On Drop the beacon is removed,
/// so peers don't have to wait for prune-on-read to forget us.
pub struct BeaconGuard {
    pid: u32,
}

impl BeaconGuard {
    pub fn register(beacon: &Beacon) -> io::Result<Self> {
        register(beacon)?;
        Ok(Self { pid: beacon.pid })
    }
}

impl Drop for BeaconGuard {
    fn drop(&mut self) {
        deregister(self.pid);
    }
}

/// Read all beacons currently on the air. Stale entries (whose pid is no
/// longer alive locally) are pruned from disk and excluded from the result.
pub fn list_active() -> Vec<Beacon> {
    let Some(dir) = radio_dir() else {
        return Vec::new();
    };
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let beacon: Beacon = match serde_json::from_slice(&bytes) {
            Ok(b) => b,
            Err(_) => {
                // Corrupt entry: drop it.
                let _ = fs::remove_file(&path);
                continue;
            }
        };
        if pid_is_alive(beacon.pid) {
            out.push(beacon);
        } else {
            let _ = fs::remove_file(&path);
        }
    }
    out
}

/// A radio message dropped into a peer's inbox. The recipient drains
/// these in `read_inbox` and removes them from disk after reading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub from_pid: u32,
    pub from_call_sign: String,
    pub sent_at_unix: u64,
    pub body: String,
}

impl Message {
    pub fn new(from_call_sign: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            from_pid: std::process::id(),
            from_call_sign: from_call_sign.into(),
            sent_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            body: body.into(),
        }
    }
}

fn inbox_dir(pid: u32) -> Option<PathBuf> {
    yarp_core::paths::yarp_home_radio_inbox_dir().map(|dir| dir.join(pid.to_string()))
}

/// Drop a message into `to_pid`'s inbox. Filename is the message's
/// `sent_at_unix` plus a nano-suffix to avoid collisions when a sender
/// fires multiple messages within the same second.
pub fn send_message(to_pid: u32, msg: &Message) -> io::Result<()> {
    let Some(dir) = inbox_dir(to_pid) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no home dir for radio inbox",
        ));
    };
    fs::create_dir_all(&dir)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let filename = format!("{}-{}-{:09}.json", msg.sent_at_unix, msg.from_pid, nanos);
    let path = dir.join(filename);
    let json =
        serde_json::to_vec_pretty(msg).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&path, json)
}

/// Drain this process's inbox: returns all pending messages in send order
/// and removes them from disk. Corrupt files are silently dropped.
pub fn read_inbox() -> Vec<Message> {
    let Some(dir) = inbox_dir(std::process::id()) else {
        return Vec::new();
    };
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut paths_and_msgs: Vec<(PathBuf, Message)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        match serde_json::from_slice::<Message>(&bytes) {
            Ok(msg) => paths_and_msgs.push((path, msg)),
            Err(_) => {
                let _ = fs::remove_file(&path);
            }
        }
    }
    paths_and_msgs.sort_by_key(|(p, _)| p.file_name().map(|s| s.to_os_string()));
    let mut out = Vec::with_capacity(paths_and_msgs.len());
    for (path, msg) in paths_and_msgs {
        let _ = fs::remove_file(&path);
        out.push(msg);
    }
    out
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    // signal 0 = existence/permission probe without delivery.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(windows)]
fn pid_is_alive(pid: u32) -> bool {
    // Best-effort fallback: assume alive on Windows. Pruning happens via
    // explicit deregister on shutdown; readers tolerate occasional stale
    // entries.
    let _ = pid;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_round_trips_through_json() {
        let beacon = Beacon::new("dev.yarp.Yarp", "Sandford");
        let json = serde_json::to_string(&beacon).unwrap();
        let back: Beacon = serde_json::from_str(&json).unwrap();
        assert_eq!(beacon, back);
    }

    #[test]
    fn beacon_without_tab_title_round_trips_legacy_json() {
        // Older beacons on disk won't have the tab_title field. Make sure
        // they still deserialize cleanly via the serde(default).
        let legacy = r#"{
            "pid": 1234,
            "started_at_unix": 0,
            "hostname": "h",
            "app_id": "dev.yarp.Yarp",
            "call_sign": "Sandford"
        }"#;
        let parsed: Beacon = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed.tab_title, None);
        assert_eq!(parsed.call_sign, "Sandford");
    }

    #[test]
    fn with_tab_title_sets_the_field() {
        let beacon = Beacon::new("dev.yarp.Yarp", "Sandford").with_tab_title("Sandford Precinct");
        assert_eq!(beacon.tab_title.as_deref(), Some("Sandford Precinct"));
    }

    #[test]
    fn format_uptime_buckets() {
        assert_eq!(format_uptime(0), "0s");
        assert_eq!(format_uptime(45), "45s");
        assert_eq!(format_uptime(125), "2m05s");
        assert_eq!(format_uptime(3700), "1h01m");
    }

    #[test]
    fn format_peer_list_empty() {
        assert_eq!(
            format_peer_list(&[]),
            "No other officers on the channel — sole patrol."
        );
    }

    #[test]
    fn format_peer_list_uses_unfiled_patrol_when_no_tab_title() {
        let summary = PeerSummary {
            pid: 7,
            call_sign: "Sandford".into(),
            tab_title: None,
            hostname: "h".into(),
            uptime_secs: 65,
        };
        let rendered = format_peer_list(std::slice::from_ref(&summary));
        assert!(rendered.contains("Sandford"));
        assert!(rendered.contains("Unfiled patrol"));
        assert!(rendered.contains("pid 7"));
        assert!(rendered.contains("1m05s"));
    }

    #[test]
    fn peer_summary_uptime_handles_clock_skew() {
        // started_at in the future => saturating sub clamps to 0 instead of
        // panicking on the underflow.
        let beacon = Beacon {
            pid: 1,
            started_at_unix: 1000,
            hostname: "h".into(),
            app_id: "dev.yarp.Yarp".into(),
            call_sign: "Sandford".into(),
            tab_title: None,
        };
        let summary = PeerSummary::from_beacon(beacon, 500);
        assert_eq!(summary.uptime_secs, 0);
    }

    #[test]
    fn message_round_trips_through_json() {
        let msg = Message::new("Sandford", "10-4");
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn message_constructor_sets_sender_pid_and_body() {
        let msg = Message::new("Sandford", "Need backup at the supermarket");
        assert_eq!(msg.from_pid, std::process::id());
        assert_eq!(msg.from_call_sign, "Sandford");
        assert_eq!(msg.body, "Need backup at the supermarket");
    }

    #[test]
    fn guard_drop_calls_deregister() {
        // The guard's Drop should not panic even if the beacon was never
        // written to disk. Use a beacon for a fake pid so we don't disturb
        // a real one if the test runs while the GUI is up.
        let guard = BeaconGuard {
            pid: 0xDEAD_BEEF,
        };
        drop(guard);
    }
}
