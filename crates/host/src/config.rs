//! Listen-address and content-directory resolution for the host binary.
//!
//! - Listen address: default `127.0.0.1:9001`, override with `GECKOSIM_HOST_ADDR`.
//!   Loopback-only at v0 (no auth, no TLS) — see ADR 0013.
//! - Content directory: default `<workspace>/content`, override with
//!   `GECKOSIM_CONTENT_DIR`.

use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

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

const DEFAULT_CONTENT_SUBDIR: &str = "content";

/// Environment variable consulted by [`content_dir`].
pub const CONTENT_ENV_VAR: &str = "GECKOSIM_CONTENT_DIR";

/// Pure helper: resolve a content directory from `Some(env_value)` or fall
/// back to the workspace-relative default. Exposed for tests; production
/// calls go through [`content_dir`].
pub fn resolve_content_dir(raw: Option<&str>) -> PathBuf {
    if let Some(s) = raw {
        return PathBuf::from(s);
    }
    // CARGO_MANIFEST_DIR = <workspace>/crates/host
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(DEFAULT_CONTENT_SUBDIR)
}

/// Resolve the content directory from `GECKOSIM_CONTENT_DIR` or fall back
/// to `<workspace>/content`. Reads process env at call time.
pub fn content_dir() -> PathBuf {
    resolve_content_dir(std::env::var(CONTENT_ENV_VAR).ok().as_deref())
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

    #[test]
    fn content_dir_default_ends_in_content() {
        let path = resolve_content_dir(None);
        assert!(
            path.ends_with("content"),
            "expected default to end in 'content', got {}",
            path.display()
        );
    }

    #[test]
    fn content_dir_override_uses_raw_path() {
        let path = resolve_content_dir(Some("/abs/path/to/elsewhere"));
        assert_eq!(path, std::path::PathBuf::from("/abs/path/to/elsewhere"));
    }
}
