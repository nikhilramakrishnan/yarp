mod config;
mod state;

use std::fmt;

pub use config::*;
pub use state::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// The open-source build of Yarp.
    Oss,

    /// The integration test build.
    Integration,
}

impl Channel {
    /// Whether or not this channel is for internal use only
    pub fn is_dogfood(&self) -> bool {
        false
    }

    /// Whether this channel honors the `--server-root-url` / `--ws-server-url` /
    /// `--session-sharing-server-url` flags (and their `YARP_*` env-var equivalents).
    ///
    /// `Oss` ignores these overrides so shipped builds can't be redirected away from
    /// their baked-in server URLs. `Integration` continues to honor them for testing.
    pub fn allows_server_url_overrides(&self) -> bool {
        matches!(self, Channel::Integration)
    }

    /// Returns the CLI command name corresponding to this channel.
    pub fn cli_command_name(&self) -> &'static str {
        match self {
            Channel::Integration => "fuzz-integration",
            Channel::Oss => "yarp",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Channel::Integration => "integration",
            Channel::Oss => "yarp",
        })
    }
}
