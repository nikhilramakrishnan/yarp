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
