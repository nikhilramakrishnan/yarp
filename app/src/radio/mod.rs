//! Inter-terminal radio: each running Yarp drops a JSON beacon at
//! `~/.yarp/radio/<pid>.json` so peers can discover it. Stale beacons (pid no
//! longer alive) are pruned on read.
//!
//! Hot Fuzz lexicon: each beacon is an officer's call sign on the channel; the
//! radio dir is the precinct's open frequency.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// Tracks "I am the one calling 10-13" — broadcast doesn't self-deliver, so
// without this flag the originator's UI would stay routine while every
// recipient lights up red. Stored as a unix-second deadline; 0 means clear.
// In-process state, not persisted: a Yarp restart drops the flag, which is
// the right default — restart implies the situation has been resolved.
static SELF_MAYDAY_UNTIL: AtomicU64 = AtomicU64::new(0);

/// Self-mayday window. Picked to outlast a normal cross-precinct response —
/// peers see the 10-13 in their inbox, ack as en-route, and the originator's
/// UI auto-clears once a "10-4" reply lands. The TTL is the safety net for
/// the case where every peer is offline; long enough to hand off to a manual
/// Stand down without auto-resolving prematurely.
pub const SELF_MAYDAY_TTL_SECS: u64 = 300;

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

/// Coarse "Nm ago" / "Nh ago" / "Nd ago" age string for a unix timestamp,
/// computed against the wall clock. Caps at "now" if the timestamp is in
/// the future (e.g. minor clock skew between sender and receiver).
pub fn format_dispatch_age(sent_at_unix: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_dispatch_age_at(sent_at_unix, now)
}

fn format_dispatch_age_at(sent_at_unix: u64, now_unix: u64) -> String {
    let secs = now_unix.saturating_sub(sent_at_unix);
    if secs < 5 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
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
    if let Some(root) = radio_root_override() {
        return Some(root);
    }
    yarp_core::paths::yarp_home_radio_dir()
}

fn beacon_path(pid: u32) -> Option<PathBuf> {
    radio_dir().map(|dir| dir.join(format!("{pid}.json")))
}

fn inbox_root() -> Option<PathBuf> {
    radio_dir().map(|dir| dir.join("inbox"))
}

// In tests, point all radio IO at a tempdir instead of the user's real
// `~/.yarp`. Thread-local so parallel tests don't collide. The override is
// `None` in production builds so this collapses to a single `if-let-some`
// branch with no overhead.
#[cfg(test)]
thread_local! {
    static RADIO_ROOT_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn radio_root_override() -> Option<PathBuf> {
    RADIO_ROOT_OVERRIDE.with(|cell| cell.borrow().clone())
}

#[cfg(not(test))]
fn radio_root_override() -> Option<PathBuf> {
    None
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
    inbox_root().map(|dir| dir.join(pid.to_string()))
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

/// All-cars dispatch: send `body` to every other live officer on the
/// channel. Returns (delivered, failed) — the two counts always sum to the
/// peer count at call time. Senders never appear in their own delivery list.
pub fn broadcast(body: impl Into<String>) -> (usize, usize) {
    let body: String = body.into();
    let msg = Message::new(self_call_sign(), body);
    let peer_list = peers();
    let mut delivered = 0usize;
    let mut failed = 0usize;
    for peer in peer_list {
        match send_message(peer.pid, &msg) {
            Ok(()) => delivered += 1,
            Err(_) => failed += 1,
        }
    }
    (delivered, failed)
}

pub fn peek_inbox() -> Vec<Message> {
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
    let raw: Vec<Message> = paths_and_msgs.into_iter().map(|(_, m)| m).collect();
    resolve_superseded_emergencies(raw)
}

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
    // Drain on disk regardless, then drop superseded emergencies from the
    // returned Vec so callers (en-route ack) don't reply 10-4 to a sender
    // whose stand-down already resolved their emergency.
    resolve_superseded_emergencies(out)
}

/// Drain only messages from the given pid — used when responding 1:1 to a
/// peer's 10-13 so the responder's UI clears that specific peer's distress
/// without dropping pending traffic from other senders. Returns the drained
/// messages in arrival order; other senders' files stay on disk untouched.
pub fn drain_from_pid(from_pid: u32) -> Vec<Message> {
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
            Ok(msg) if msg.from_pid == from_pid => paths_and_msgs.push((path, msg)),
            Ok(_) => {}
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


/// Drain en-route replies from this Yarp's inbox — used at stand-down so the
/// originator's inbox doesn't keep displaying acks to a now-resolved 10-13.
/// Other senders' messages stay put; only en-route bodies are reaped.
pub fn drain_en_route_replies() -> usize {
    let Some(dir) = inbox_dir(std::process::id()) else {
        return 0;
    };
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    let mut reaped = 0usize;
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
            Ok(msg) if is_en_route_body(&msg.body) => {
                if fs::remove_file(&path).is_ok() {
                    reaped += 1;
                }
            }
            Ok(_) => {}
            Err(_) => {
                let _ = fs::remove_file(&path);
            }
        }
    }
    reaped
}

/// Most recent dispatch in this process's inbox, or None if empty. Reads
/// without draining — UI surfaces can render the latest body alongside a
/// pending-count without consuming the message.
///
/// Emergency promotion: if any queued message is a 10-13 we surface the
/// latest 10-13 instead of the wall-clock latest. Without this, a routine
/// hail arriving after a 10-13 would mask the emergency in the dispatch row
/// even though the inbox-line and roster still flagged it red — split-signal
/// bug. Within urgent vs. routine groups, newest still wins.
pub fn latest_dispatch() -> Option<Message> {
    let inbox = peek_inbox();
    if let Some(latest_emergency) = inbox
        .iter()
        .filter(|m| is_emergency_body(&m.body))
        .max_by_key(|m| m.sent_at_unix)
    {
        return Some(latest_emergency.clone());
    }
    inbox.into_iter().max_by_key(|m| m.sent_at_unix)
}


/// Classify a message body as a 10-13 (officer needs assistance) emergency.
/// Detected on the raw body so any UI surface or workspace handler can branch
/// on urgency without re-implementing the parser. Police shorthand: 10-13
/// always leads the body when broadcast or hailed via the emergency CTA.
pub fn is_emergency_body(body: &str) -> bool {
    body.trim_start().starts_with("10-13")
}

/// Classify a message body as a stand-down — the originator declaring their
/// 10-13 resolved. Receivers use this to hide superseded emergencies from
/// the same sender so the UI reflects "situation handled" instead of staying
/// red until manually acked.
pub fn is_stand_down_body(body: &str) -> bool {
    body.trim() == STAND_DOWN_BROADCAST_BODY.trim()
}


/// Classify a message body as an "en route" reply — the canonical 1:1
/// response to a 10-13 hail. Lets the inbox roster line frame an inbox of
/// pure en-route replies as backup converging rather than generic traffic.
pub fn is_en_route_body(body: &str) -> bool {
    body.trim() == EN_ROUTE_BODY.trim()
}

/// Classify a message body as a routine hail — the standard "checking in"
/// 1:1 ping. UI surfaces collapse the verbatim "Hail — checking in." body
/// into a calmer "Hail" lead so the dispatch row reads as a quiet roll-call
/// rather than a quoted snippet, mirroring how stand-downs collapse.
pub fn is_hail_body(body: &str) -> bool {
    body.trim() == HAIL_BODY.trim()
}

/// Filter superseded emergencies: when a sender's stand-down arrives after
/// their 10-13, the 10-13 is no longer urgent — the situation resolved on
/// the originator's side. Walks forward (messages are arrival-sorted), tracks
/// each sender's latest stand-down, and drops 10-13s from that sender at or
/// before that timestamp. The stand-down message itself stays so the receiver
/// still sees the resolution as latest dispatch.
fn resolve_superseded_emergencies(mut msgs: Vec<Message>) -> Vec<Message> {
    use std::collections::HashMap;
    let mut latest_stand_down: HashMap<u32, u64> = HashMap::new();
    for m in &msgs {
        if is_stand_down_body(&m.body) {
            let entry = latest_stand_down.entry(m.from_pid).or_insert(0);
            if m.sent_at_unix > *entry {
                *entry = m.sent_at_unix;
            }
        }
    }
    msgs.retain(|m| {
        if !is_emergency_body(&m.body) {
            return true;
        }
        match latest_stand_down.get(&m.from_pid) {
            Some(&t) => m.sent_at_unix > t,
            None => true,
        }
    });
    msgs
}


/// Mark this Yarp as actively calling 10-13 for `ttl` seconds. Lets the
/// originator's UI render "calling 10-13" while their broadcast is in flight
/// — without it, the broadcaster's signon would stay "on patrol" because
/// broadcast doesn't self-deliver. Also flips when the originator manually
/// stands down by clearing the flag.
pub fn mark_self_mayday(ttl: Duration) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    SELF_MAYDAY_UNTIL.store(now + ttl.as_secs(), Ordering::Relaxed);
}

/// Drop the self-mayday flag (e.g. when the operator stands down or
/// receives an "en route" reply).
pub fn clear_self_mayday() {
    SELF_MAYDAY_UNTIL.store(0, Ordering::Relaxed);
}

// Tracks when this Yarp last fired a stand-down so the signon line can
// render a brief "stood down · channel clear" acknowledgment instead of
// jumping straight back to routine "on patrol". Without this hook the
// stand-down click is silent — the originator sees no transient confirmation
// that the channel heard them. Stored as the unix-second timestamp of the
// stand-down event; 0 means none recorded. In-process only, like the
// mayday flag — a restart implies acknowledgment has long since been read.
static SELF_STAND_DOWN_AT: AtomicU64 = AtomicU64::new(0);

/// How long the post-stand-down acknowledgment lingers on the signon line.
/// Picked so the operator gets a clear "channel heard you" beat after the
/// click before the row settles back to routine — long enough to read,
/// short enough that it doesn't squat on top of fresh inbox traffic.
pub const SELF_STAND_DOWN_ACK_SECS: u64 = 30;

/// Mark that this Yarp just fired a stand-down. Drives the transient
/// signon ack window — the radio handlers in workspace::view call this
/// the moment the stand-down broadcast goes out.
pub fn mark_self_stand_down() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    SELF_STAND_DOWN_AT.store(now, Ordering::Relaxed);
}

/// Seconds since the last stand-down, while still inside the ack window;
/// `None` once the window expires (or if no stand-down has been recorded).
/// UI surfaces gate the "stood down · channel clear" line on `Some(_)`.
pub fn time_since_self_stand_down() -> Option<Duration> {
    let at = SELF_STAND_DOWN_AT.load(Ordering::Relaxed);
    if at == 0 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let elapsed = now.saturating_sub(at);
    if elapsed >= SELF_STAND_DOWN_ACK_SECS {
        return None;
    }
    Some(Duration::from_secs(elapsed))
}

// Tracks when this Yarp last fired a mic check so the signon line can
// render a brief "mic check · stand by" ack instead of the click being
// silent. Mic-check is a question to the channel ("anyone on?"), so the
// ack window confirms the broadcast left the building before any peer
// reply lands. Stored as the unix-second timestamp; 0 means none recorded.
// In-process only, like the mayday and stand-down flags.
static SELF_MIC_CHECK_AT: AtomicU64 = AtomicU64::new(0);

/// How long the post-mic-check acknowledgment lingers on the signon line.
/// Shorter than the stand-down ack — a mic-check is a routine "you there?"
/// not a closure of distress, so the beat just needs to confirm the
/// broadcast went out before settling back to routine.
pub const SELF_MIC_CHECK_ACK_SECS: u64 = 15;

/// Mark that this Yarp just fired a mic check. Drives the transient
/// signon ack window; called from the radio handler the moment the
/// mic-check broadcast goes out.
pub fn mark_self_mic_check() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    SELF_MIC_CHECK_AT.store(now, Ordering::Relaxed);
}

/// Seconds since the last mic check, while still inside the ack window;
/// `None` once the window expires (or if no mic-check has been recorded).
/// UI surfaces gate the "mic check · stand by" line on `Some(_)`.
pub fn time_since_self_mic_check() -> Option<Duration> {
    let at = SELF_MIC_CHECK_AT.load(Ordering::Relaxed);
    if at == 0 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let elapsed = now.saturating_sub(at);
    if elapsed >= SELF_MIC_CHECK_ACK_SECS {
        return None;
    }
    Some(Duration::from_secs(elapsed))
}

/// True if this Yarp is currently flagged as calling 10-13 (TTL not yet
/// expired). UI surfaces use this to render the originator's status row in
/// the same red rhythm as the responder side.
pub fn self_in_mayday() -> bool {
    let until = SELF_MAYDAY_UNTIL.load(Ordering::Relaxed);
    if until == 0 {
        return false;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    now < until
}

/// Unix-second timestamp when this Yarp began calling 10-13, derived from the
/// stored deadline minus the fixed TTL. None when not in mayday or after
/// expiry. Lets dispatch surfaces format "broadcasting · 12s ago" with the
/// same age helper used for inbox messages, so self-broadcast and peer reply
/// share visual rhythm.
pub fn self_mayday_started_at_unix() -> Option<u64> {
    let until = SELF_MAYDAY_UNTIL.load(Ordering::Relaxed);
    if until == 0 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    if now >= until {
        return None;
    }
    Some(until.saturating_sub(SELF_MAYDAY_TTL_SECS))
}


// Canonical message bodies for the precinct radio. Centralized so a hail, the
// emergency response, the routine ack-on-emergency, and any future palette
// entry all speak the same wire format — divergence here would let two yarps
// disagree on what counts as a hail vs. an emergency reply.
pub const HAIL_BODY: &str = "Hail — checking in.";
pub const EN_ROUTE_BODY: &str = "10-4, en route — hold tight.";
pub const MIC_CHECK_BROADCAST_BODY: &str = "Mic check — anyone on this channel?";
pub const TEN_THIRTEEN_BROADCAST_BODY: &str = "10-13! Officer needs assistance — copy and respond.";
pub const STAND_DOWN_BROADCAST_BODY: &str = "Stand down — situation resolved.";

/// Sweep `~/.yarp/radio/inbox/<pid>/` directories whose owning pid is no
/// longer alive — without it, a long-running install accumulates inbox dirs
/// for every terminal that ever booted. Returns the number of inboxes
/// reclaimed. Best-effort; IO failures are silently ignored.
pub fn prune_dead_inboxes() -> usize {
    let Some(root) = inbox_root() else {
        return 0;
    };
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    let mut reclaimed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(pid) = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid_is_alive(pid) {
            continue;
        }
        if fs::remove_dir_all(&path).is_ok() {
            reclaimed += 1;
        }
    }
    reclaimed
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

    /// RAII helper: install a tempdir as the radio root and clear on Drop.
    /// Wraps `tempfile::TempDir` so the directory is also wiped at end of
    /// scope. Test-only.
    struct RadioSandbox {
        _dir: tempfile::TempDir,
    }

    impl RadioSandbox {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            RADIO_ROOT_OVERRIDE.with(|cell| {
                *cell.borrow_mut() = Some(dir.path().to_path_buf());
            });
            Self { _dir: dir }
        }
    }

    impl Drop for RadioSandbox {
        fn drop(&mut self) {
            RADIO_ROOT_OVERRIDE.with(|cell| *cell.borrow_mut() = None);
        }
    }

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
    fn format_dispatch_age_buckets() {
        assert_eq!(format_dispatch_age_at(100, 100), "just now");
        assert_eq!(format_dispatch_age_at(100, 104), "just now");
        assert_eq!(format_dispatch_age_at(100, 110), "10s ago");
        assert_eq!(format_dispatch_age_at(0, 125), "2m ago");
        assert_eq!(format_dispatch_age_at(0, 3700), "1h ago");
        assert_eq!(format_dispatch_age_at(0, 90_000), "1d ago");
        // Future-dated message (clock skew) caps at "just now".
        assert_eq!(format_dispatch_age_at(200, 100), "just now");
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

    #[test]
    fn broadcast_with_no_peers_returns_zero_zero() {
        // Self is filtered out of peers(), so on a one-officer channel a
        // broadcast must report zero attempts — never one self-delivery.
        let (delivered, failed) = broadcast("All-cars test ping");
        assert_eq!(delivered + failed, peers().len());
    }

    #[test]
    fn latest_dispatch_is_safe_when_inbox_empty() {
        // No messages in this pid's inbox => None, never a panic, never
        // a fabricated empty Message.
        let _ = latest_dispatch();
    }

    #[test]
    fn prune_dead_inboxes_is_safe_when_root_missing() {
        // Pruning must not panic when the inbox root has not yet been
        // created on disk (e.g. fresh install where nobody's sent a message).
        let _ = prune_dead_inboxes();
    }

    #[test]
    fn register_then_list_active_round_trips_in_sandbox() {
        let _sandbox = RadioSandbox::new();
        let beacon = Beacon::new("dev.yarp.Yarp", "Sandford")
            .with_tab_title("Test patrol");
        // Use the current pid so the alive-check doesn't filter us out.
        let mut beacon = beacon;
        beacon.pid = std::process::id();
        register(&beacon).expect("register");

        let active = list_active();
        let mine = active.iter().find(|b| b.pid == beacon.pid).expect("self");
        assert_eq!(mine.call_sign, "Sandford");
        assert_eq!(mine.tab_title.as_deref(), Some("Test patrol"));
    }

    #[test]
    fn list_active_drops_dead_pid_beacons() {
        let _sandbox = RadioSandbox::new();
        // Fabricate a beacon for a pid that's almost certainly not alive.
        let mut beacon = Beacon::new("dev.yarp.Yarp", "Ghost");
        beacon.pid = 0xDEAD_BEEF;
        register(&beacon).expect("register dead");
        let path = beacon_path(beacon.pid).expect("beacon path");
        assert!(path.exists(), "dead beacon should land on disk first");

        let active = list_active();
        assert!(active.iter().all(|b| b.pid != beacon.pid));
        assert!(!path.exists(), "list_active should reap the dead beacon");
    }

    #[test]
    fn send_message_then_peek_returns_it_in_sandbox() {
        let _sandbox = RadioSandbox::new();
        let to = std::process::id();
        let msg = Message::new("Sandford", "Hoggett's on the loose");
        send_message(to, &msg).expect("send");

        let pending = peek_inbox();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].body, "Hoggett's on the loose");
        assert_eq!(pending[0].from_call_sign, "Sandford");
    }

    #[test]
    fn read_inbox_drains_messages_in_sandbox() {
        let _sandbox = RadioSandbox::new();
        let to = std::process::id();
        send_message(to, &Message::new("Sandford", "first")).expect("send 1");
        // Different sender pid avoids the per-second nano-suffix collision.
        let mut second = Message::new("Butterman", "second");
        second.from_pid = std::process::id().wrapping_add(1);
        send_message(to, &second).expect("send 2");

        let drained = read_inbox();
        assert_eq!(drained.len(), 2);
        assert!(read_inbox().is_empty(), "second drain must be empty");
    }

    #[test]
    fn prune_dead_inboxes_removes_dead_pid_dirs_in_sandbox() {
        let _sandbox = RadioSandbox::new();
        // Fabricate a dead-pid inbox dir with a stub message inside.
        let dead_pid: u32 = 0xDEAD_BEEF;
        let dead_dir = inbox_dir(dead_pid).expect("dead dir");
        fs::create_dir_all(&dead_dir).expect("mkdir");
        fs::write(dead_dir.join("0-0-000000000.json"), b"{}").expect("write stub");

        // Live pid (current process) should survive the sweep.
        let live_dir = inbox_dir(std::process::id()).expect("live dir");
        fs::create_dir_all(&live_dir).expect("mkdir live");

        let reclaimed = prune_dead_inboxes();
        assert_eq!(reclaimed, 1);
        assert!(!dead_dir.exists());
        assert!(live_dir.exists());
    }
}
