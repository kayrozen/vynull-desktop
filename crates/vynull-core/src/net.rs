//! Host checks for the connection screen. The API has no authentication, so
//! the client warns when the daemon is not on a private network.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// True for loopback, link-local, private-range and `.local` hosts. Any other
/// host name or address is treated as public.
pub fn is_private_host(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => private_v4(ip),
        Ok(IpAddr::V6(ip)) => private_v6(ip),
        Err(_) => false,
    }
}

fn private_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private() || ip.is_link_local()
}

fn private_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
}

/// Split `host[:port]` (also `[v6]:port`); the port defaults to the daemon's 9443.
pub fn split_host_port(input: &str) -> Option<(String, u16)> {
    let input = input
        .trim()
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if input.is_empty() {
        return None;
    }
    if let Some(rest) = input.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = match tail.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None if tail.is_empty() => 9443,
            None => return None,
        };
        return Some((format!("[{host}]"), port));
    }
    match input.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !h.contains(':') => Some((h.to_string(), p.parse().ok()?)),
        Some(_) => None,
        None => Some((input.to_string(), 9443)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_and_public_hosts() {
        for h in [
            "127.0.0.1",
            "192.168.1.10",
            "10.0.0.5",
            "172.16.3.1",
            "169.254.20.7",
            "localhost",
            "booth.local",
            "::1",
            "fd00::1",
            "fe80::1",
        ] {
            assert!(is_private_host(h), "{h}");
        }
        for h in ["8.8.8.8", "172.32.0.1", "example.com", "2001:db8::1"] {
            assert!(!is_private_host(h), "{h}");
        }
    }

    #[test]
    fn host_port_parsing() {
        assert_eq!(
            split_host_port("192.168.1.10"),
            Some(("192.168.1.10".into(), 9443))
        );
        assert_eq!(
            split_host_port("http://pi.local:8000/"),
            Some(("pi.local".into(), 8000))
        );
        assert_eq!(
            split_host_port("[fe80::1]:9443"),
            Some(("[fe80::1]".into(), 9443))
        );
        assert_eq!(split_host_port("host:abc"), None);
        assert_eq!(split_host_port(""), None);
    }
}
