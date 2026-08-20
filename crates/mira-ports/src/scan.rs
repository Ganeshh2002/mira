//! Listening sockets.
//!
//! Read through `netstat2`, which uses each platform's native interface —
//! `sysctl` on macOS, `GetExtendedTcpTable` on Windows, `/proc/net` plus netlink
//! on Linux. No command is run and no human-readable output is parsed, which
//! rules out both the injection surface and the fragility of screen-scraping
//! `lsof` (`security-and-privacy.md` §5).

use mira_core::{MiraError, Result};
use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One socket accepting connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Listener {
    /// The port it is listening on.
    #[ts(type = "number")]
    pub port: u16,
    /// The address it is bound to, as a person would type it.
    pub local_address: String,
    /// The process that owns it, where the operating system says.
    ///
    /// `None` happens: a socket owned by another user, or one whose process
    /// exited between the scan and the read.
    #[ts(type = "number | null")]
    pub pid: Option<u32>,
}

/// What Mira may ask about sockets.
pub trait PortScanner {
    /// Every TCP socket in the listening state.
    ///
    /// # Errors
    ///
    /// [`MiraError::PermissionDenied`] where the platform refuses the read, and
    /// [`MiraError::External`] for anything else it reports.
    fn listening(&self) -> Result<Vec<Listener>>;
}

/// The real socket table.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ports;

impl Ports {
    /// A scanner reading this machine.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl PortScanner for Ports {
    fn listening(&self) -> Result<Vec<Listener>> {
        // TCP only. A UDP socket has no listening state, so "what is serving on
        // :3000" has no UDP answer to give.
        let sockets = netstat2::get_sockets_info(
            AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
            ProtocolFlags::TCP,
        )
        .map_err(describe)?;

        let mut listeners: Vec<Listener> = sockets
            .into_iter()
            .filter_map(|socket| {
                let ProtocolSocketInfo::Tcp(tcp) = socket.protocol_socket_info else {
                    return None;
                };
                if tcp.state != TcpState::Listen {
                    return None;
                }
                Some(Listener {
                    port: tcp.local_port,
                    local_address: tcp.local_addr.to_string(),
                    // A socket can be reported with several owning pids. The
                    // first is the one Mira attributes by; claiming a set would
                    // be more precise and less useful.
                    pid: socket.associated_pids.first().copied(),
                })
            })
            .collect();

        // Sorted and deduplicated so the list does not reshuffle between reads.
        // A port bound on both IPv4 and IPv6 is one service, and showing it twice
        // would read as two.
        listeners.sort_by(|a, b| {
            (a.port, &a.local_address, a.pid).cmp(&(b.port, &b.local_address, b.pid))
        });
        listeners.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);

        Ok(listeners)
    }
}

fn describe(error: impl std::fmt::Display) -> MiraError {
    let detail = error.to_string();

    if detail.to_lowercase().contains("permission") || detail.contains("EPERM") {
        return MiraError::PermissionDenied {
            what: "Reading the list of listening ports".to_owned(),
            hint: "Mira can still show everything else about your projects.".to_owned(),
        };
    }

    MiraError::external("the network socket table", detail)
}
