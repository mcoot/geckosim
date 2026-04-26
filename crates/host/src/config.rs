//! Listen-address resolution for the host binary.
//!
//! Default: `127.0.0.1:9001`. Override with `GECKOSIM_HOST_ADDR=…`.
//! Loopback-only at v0 (no auth, no TLS) — see ADR 0013.

use std::env;
use std::net::SocketAddr;

const DEFAULT_ADDR: &str = "127.0.0.1:9001";

/// Environment variable consulted by [`listen_addr`].
pub const ENV_VAR: &str = "GECKOSIM_HOST_ADDR";

/// Pure helper: parse a `SocketAddr` from `Some(env_value)` or fall back
/// to the v0 default. Exposed for tests; production calls use [`listen_addr`].
pub fn parse_addr(raw: Option<&str>) -> anyhow::Result<SocketAddr> {
    let s = raw.unwrap_or(DEFAULT_ADDR);
    s.parse::<SocketAddr>()
        .map_err(|e| anyhow::anyhow!("invalid {ENV_VAR}={s:?}: {e}"))
}

/// Resolve the listen address from `GECKOSIM_HOST_ADDR` or fall back to
/// `127.0.0.1:9001`. Reads process env at call time.
pub fn listen_addr() -> anyhow::Result<SocketAddr> {
    parse_addr(env::var(ENV_VAR).ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_loopback_9001() {
        let addr = parse_addr(None).expect("parse default");
        assert_eq!(addr.to_string(), "127.0.0.1:9001");
    }

    #[test]
    fn override_with_ephemeral_port_parses() {
        let addr = parse_addr(Some("127.0.0.1:0")).expect("parse override");
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 0);
    }

    #[test]
    fn invalid_addr_returns_err() {
        let err = parse_addr(Some("not a socket addr")).expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains(ENV_VAR), "msg = {msg}");
    }
}
