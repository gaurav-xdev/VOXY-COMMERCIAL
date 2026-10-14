//! SSRF & DNS Rebinding Defense Engine
//!
//! Enforces destination URL and IP address validation at socket connection points,
//! strictly rejecting:
//! - Loopback destinations (`127.0.0.0/8`, `::1`, IPv4-mapped loopback)
//! - Link-local destinations (`169.254.0.0/16`, `fe80::/10`)
//! - Private networks (RFC 1918: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`)
//! - Cloud instance metadata endpoints (`169.254.169.254`, `169.254.170.2`, `100.100.100.200`, `fd00:ec2::254`, `metadata.google.internal`)
//! - Non-routable and reserved ranges (CGNAT `100.64.0.0/10`, broadcast `255.255.255.255`, unspecified, multicast, documentation)
//! - Non-HTTP/HTTPS schemes (file, gopher, ftp, ldap, dict, etc.)
//!
//! Pins resolved IP addresses to protect against DNS rebinding (TOCTOU attacks)
//! before establishing TCP socket connections.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::time::Duration;
use url::Url;

use crate::error::SecurityError;

/// Determines whether an IP address belongs to a private, loopback, link-local,
/// cloud metadata, multicast, or non-routable range.
pub fn is_ip_restricted(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_ipv4_restricted(&v4),
        IpAddr::V6(v6) => is_ipv6_restricted(&v6),
    }
}

/// Validates an IPv4 address against SSRF policies.
pub fn is_ipv4_restricted(v4: &Ipv4Addr) -> bool {
    let octets = v4.octets();

    // Loopback: 127.0.0.0/8
    if v4.is_loopback() || octets[0] == 127 {
        return true;
    }

    // Link-local: 169.254.0.0/16 (includes AWS/GCP/Azure IMDS 169.254.169.254, ECS 169.254.170.2)
    if v4.is_link_local() || (octets[0] == 169 && octets[1] == 254) {
        return true;
    }

    // RFC 1918 Private Subnets:
    // 10.0.0.0/8
    if octets[0] == 10 {
        return true;
    }
    // 172.16.0.0/12 (172.16.0.0 - 172.31.255.255)
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }
    // 192.168.0.0/16
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }

    // Shared Address Space / CGNAT: 100.64.0.0/10 (100.64.0.0 - 100.127.255.255)
    // (Also includes Alibaba Cloud metadata 100.100.100.200)
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return true;
    }

    // Unspecified / "This host": 0.0.0.0/8
    if v4.is_unspecified() || octets[0] == 0 {
        return true;
    }

    // Broadcast: 255.255.255.255
    if v4.is_broadcast() {
        return true;
    }

    // Multicast: 224.0.0.0/4 (224.0.0.0 - 239.255.255.255)
    if v4.is_multicast() || (224..=239).contains(&octets[0]) {
        return true;
    }

    // Reserved for future use: 240.0.0.0/4
    if octets[0] >= 240 {
        return true;
    }

    // IETF Protocol Assignments: 192.0.0.0/24
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        return true;
    }

    // Documentation subnets (RFC 5737):
    // TEST-NET-1: 192.0.2.0/24
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 2 {
        return true;
    }
    // TEST-NET-2: 198.51.100.0/24
    if octets[0] == 198 && octets[1] == 51 && octets[2] == 100 {
        return true;
    }
    // TEST-NET-3: 203.0.113.0/24
    if octets[0] == 203 && octets[1] == 0 && octets[2] == 113 {
        return true;
    }

    // Benchmarking tests (RFC 2544): 198.18.0.0/15 (198.18.0.0 - 198.19.255.255)
    if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
        return true;
    }

    false
}

/// Validates an IPv6 address against SSRF policies.
pub fn is_ipv6_restricted(v6: &Ipv6Addr) -> bool {
    // Unspecified: ::
    if v6.is_unspecified() {
        return true;
    }

    // Loopback: ::1
    if v6.is_loopback() {
        return true;
    }

    let segments = v6.segments();

    // IPv4-mapped IPv6 (::ffff:x.x.x.x)
    if segments[0] == 0
        && segments[1] == 0
        && segments[2] == 0
        && segments[3] == 0
        && segments[4] == 0
        && segments[5] == 0xffff
    {
        let octet0 = (segments[6] >> 8) as u8;
        let octet1 = (segments[6] & 0xff) as u8;
        let octet2 = (segments[7] >> 8) as u8;
        let octet3 = (segments[7] & 0xff) as u8;
        let v4 = Ipv4Addr::new(octet0, octet1, octet2, octet3);
        return is_ipv4_restricted(&v4);
    }

    // IPv4-compatible IPv6 (::x.x.x.x)
    if segments[0] == 0
        && segments[1] == 0
        && segments[2] == 0
        && segments[3] == 0
        && segments[4] == 0
        && segments[5] == 0
        && segments[6] != 0
    {
        let octet0 = (segments[6] >> 8) as u8;
        let octet1 = (segments[6] & 0xff) as u8;
        let octet2 = (segments[7] >> 8) as u8;
        let octet3 = (segments[7] & 0xff) as u8;
        let v4 = Ipv4Addr::new(octet0, octet1, octet2, octet3);
        return is_ipv4_restricted(&v4);
    }

    // Unique Local Addresses (ULA): fc00::/7 (RFC 4193: fc00:: to fdff:ffff:...)
    // (Also covers AWS IPv6 IMDS fd00:ec2::254)
    if (segments[0] & 0xfe00) == 0xfc00 {
        return true;
    }

    // Link-local unicast: fe80::/10 (RFC 4291: fe80:: to febf:ffff:...)
    if (segments[0] & 0xffc0) == 0xfe80 {
        return true;
    }

    // Multicast: ff00::/8
    if v6.is_multicast() || (segments[0] & 0xff00) == 0xff00 {
        return true;
    }

    // Discard prefix: 100::/64 (RFC 6666)
    if segments[0] == 0x0100 && segments[1] == 0 && segments[2] == 0 && segments[3] == 0 {
        return true;
    }

    // Documentation: 2001:db8::/32 (RFC 3849)
    if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        return true;
    }

    // IPv4/IPv6 translation prefix: 64:ff9b::/96 (RFC 6052)
    if segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 0 && segments[3] == 0 && segments[4] == 0 && segments[5] == 0 {
        let octet0 = (segments[6] >> 8) as u8;
        let octet1 = (segments[6] & 0xff) as u8;
        let octet2 = (segments[7] >> 8) as u8;
        let octet3 = (segments[7] & 0xff) as u8;
        let v4 = Ipv4Addr::new(octet0, octet1, octet2, octet3);
        return is_ipv4_restricted(&v4);
    }

    false
}

/// Checks whether a host string is inherently restricted by name.
pub fn is_host_restricted(host: &str) -> bool {
    let lower = host.trim().to_ascii_lowercase();

    // Exact matches
    if lower == "localhost"
        || lower == "metadata"
        || lower == "metadata.google.internal"
        || lower == "metadata.google"
        || lower == "instance-data"
        || lower == "instance-data.ec2.internal"
    {
        return true;
    }

    // Suffix matches for internal/local namespaces
    if lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower.ends_with(".corp")
        || lower.ends_with(".lan")
    {
        return true;
    }

    false
}

/// Validates a parsed URL against SSRF and DNS rebinding risks.
///
/// Ensures:
/// 1. Scheme is either `http` or `https`.
/// 2. Host is present and not on the restricted names list.
/// 3. Host IP (literal or resolved via DNS) does not point to loopback,
///    link-local, cloud metadata, RFC 1918 private subnets, or reserved ranges.
///
/// Returns the first validated `SocketAddr` for the connection, pinning the IP
/// and preventing DNS rebinding (TOCTOU) attacks at connection time.
pub fn validate_destination_url(url: &Url) -> Result<SocketAddr, SecurityError> {
    // 1. Validate Scheme
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(SecurityError::InvalidDestination(format!(
            "Protocol scheme '{scheme}' is forbidden. Only HTTP and HTTPS are permitted."
        )));
    }

    // 2. Validate Host presence
    let host_str = match url.host_str() {
        Some(h) if !h.trim().is_empty() => h.trim(),
        _ => {
            return Err(SecurityError::InvalidDestination(
                "Destination URL must specify a non-empty host component".into(),
            ));
        }
    };

    // 3. Check restricted hostnames
    if is_host_restricted(host_str) {
        return Err(SecurityError::SsrfBlocked(format!(
            "Destination host '{host_str}' is restricted by SSRF defense policy"
        )));
    }

    let port = url.port_or_known_default().unwrap_or(80);

    // 4. IP literal check
    if let Ok(ip) = host_str.parse::<IpAddr>() {
        if is_ip_restricted(ip) {
            return Err(SecurityError::SsrfBlocked(format!(
                "Destination IP address '{ip}' belongs to a private, loopback, or metadata subnet"
            )));
        }
        return Ok(SocketAddr::new(ip, port));
    }

    // 5. Hostname DNS resolution and rebinding validation
    let resolved = safe_resolve_destination(host_str, port)?;
    match resolved.first() {
        Some(&addr) => Ok(addr),
        None => Err(SecurityError::InvalidDestination(format!(
            "Hostname '{host_str}' resolved to zero IP addresses"
        ))),
    }
}

/// Helper to parse and validate a URL string in one step.
pub fn validate_destination_url_str(url_str: &str) -> Result<SocketAddr, SecurityError> {
    let url = Url::parse(url_str)
        .map_err(|e| SecurityError::InvalidDestination(format!("Malformed URL '{url_str}': {e}")))?;
    validate_destination_url(&url)
}

/// Resolves a hostname to socket addresses and verifies that EVERY resolved
/// address satisfies all SSRF safety rules.
pub fn safe_resolve_destination(host: &str, port: u16) -> Result<Vec<SocketAddr>, SecurityError> {
    let socket_query = format!("{}:{}", host, port);
    let addrs = socket_query.to_socket_addrs().map_err(|e| {
        SecurityError::InvalidDestination(format!("DNS resolution failed for '{host}': {e}"))
    })?;

    let mut safe_addrs = Vec::new();
    for addr in addrs {
        if is_ip_restricted(addr.ip()) {
            return Err(SecurityError::SsrfBlocked(format!(
                "Host '{host}' resolved to restricted IP '{}' (SSRF / DNS rebinding blocked)",
                addr.ip()
            )));
        }
        safe_addrs.push(addr);
    }

    if safe_addrs.is_empty() {
        return Err(SecurityError::InvalidDestination(format!(
            "Host '{host}' did not yield any socket addresses"
        )));
    }

    Ok(safe_addrs)
}

/// Validates a raw `SocketAddr` at the actual socket creation point.
pub fn validate_socket_addr(addr: &SocketAddr) -> Result<(), SecurityError> {
    if is_ip_restricted(addr.ip()) {
        Err(SecurityError::SsrfBlocked(format!(
            "Socket destination '{}' is in a restricted IP range",
            addr.ip()
        )))
    } else {
        Ok(())
    }
}

/// Establishes a protected TCP socket connection to a destination URL.
///
/// Resolves DNS once, validates all resolved IP addresses, and connects directly
/// to the pinned `SocketAddr` within the specified timeout.
pub async fn safe_connect_tcp(
    url: &Url,
    timeout_duration: Duration,
) -> Result<(tokio::net::TcpStream, SocketAddr), SecurityError> {
    let target_addr = validate_destination_url(url)?;

    // Double-check the socket address immediately before connection
    validate_socket_addr(&target_addr)?;

    let stream = tokio::time::timeout(timeout_duration, tokio::net::TcpStream::connect(target_addr))
        .await
        .map_err(|_| {
            SecurityError::Internal(format!("Connection to destination {target_addr} timed out"))
        })?
        .map_err(|e| {
            SecurityError::Internal(format!("Failed to connect to destination {target_addr}: {e}"))
        })?;

    Ok((stream, target_addr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_loopback_ipv4() {
        assert!(is_ip_restricted("127.0.0.1".parse().unwrap()));
        assert!(is_ip_restricted("127.0.0.2".parse().unwrap()));
        assert!(is_ip_restricted("127.255.255.254".parse().unwrap()));

        assert!(validate_destination_url_str("http://127.0.0.1:8080/test").is_err());
        assert!(validate_destination_url_str("https://127.0.0.1/").is_err());
    }

    #[test]
    fn test_rejects_loopback_ipv6() {
        assert!(is_ip_restricted("::1".parse().unwrap()));
        assert!(validate_destination_url_str("http://[::1]:8080").is_err());
        assert!(validate_destination_url_str("https://[::1]/").is_err());
    }

    #[test]
    fn test_rejects_ipv4_mapped_ipv6_loopback_and_private() {
        // ::ffff:127.0.0.1
        let mapped_loopback: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert!(is_ip_restricted(mapped_loopback));
        assert!(validate_destination_url_str("http://[::ffff:127.0.0.1]:80/").is_err());

        // ::ffff:10.0.0.1
        let mapped_private: IpAddr = "::ffff:10.0.0.1".parse().unwrap();
        assert!(is_ip_restricted(mapped_private));

        // ::ffff:169.254.169.254
        let mapped_metadata: IpAddr = "::ffff:169.254.169.254".parse().unwrap();
        assert!(is_ip_restricted(mapped_metadata));
    }

    #[test]
    fn test_rejects_link_local_and_cloud_metadata() {
        // AWS/GCP/Azure IMDS
        assert!(is_ip_restricted("169.254.169.254".parse().unwrap()));
        assert!(validate_destination_url_str("http://169.254.169.254/latest/meta-data").is_err());

        // AWS ECS Task Metadata
        assert!(is_ip_restricted("169.254.170.2".parse().unwrap()));
        assert!(validate_destination_url_str("http://169.254.170.2/v2/metadata").is_err());

        // Alibaba Cloud Metadata
        assert!(is_ip_restricted("100.100.100.200".parse().unwrap()));
        assert!(validate_destination_url_str("http://100.100.100.200/latest/meta-data").is_err());

        // IPv6 Link-Local
        assert!(is_ip_restricted("fe80::1".parse().unwrap()));
        assert!(validate_destination_url_str("http://[fe80::1]:80").is_err());

        // AWS IPv6 IMDS (ULA)
        assert!(is_ip_restricted("fd00:ec2::254".parse().unwrap()));
        assert!(validate_destination_url_str("http://[fd00:ec2::254]/").is_err());
    }

    #[test]
    fn test_rejects_private_subnets_rfc1918() {
        // 10.0.0.0/8
        assert!(is_ip_restricted("10.0.0.1".parse().unwrap()));
        assert!(is_ip_restricted("10.254.12.34".parse().unwrap()));
        assert!(validate_destination_url_str("http://10.0.0.1:8000").is_err());

        // 172.16.0.0/12
        assert!(is_ip_restricted("172.16.0.1".parse().unwrap()));
        assert!(is_ip_restricted("172.24.10.5".parse().unwrap()));
        assert!(is_ip_restricted("172.31.255.254".parse().unwrap()));
        assert!(validate_destination_url_str("http://172.16.0.5:3000").is_err());

        // Boundary checks for 172.16.0.0/12
        assert!(!is_ip_restricted("172.15.255.255".parse().unwrap()));
        assert!(!is_ip_restricted("172.32.0.1".parse().unwrap()));

        // 192.168.0.0/16
        assert!(is_ip_restricted("192.168.1.1".parse().unwrap()));
        assert!(is_ip_restricted("192.168.0.254".parse().unwrap()));
        assert!(validate_destination_url_str("http://192.168.1.1:8080").is_err());
    }

    #[test]
    fn test_rejects_cgnat_and_other_reserved_ranges() {
        // CGNAT: 100.64.0.0/10
        assert!(is_ip_restricted("100.64.0.1".parse().unwrap()));
        assert!(is_ip_restricted("100.127.255.254".parse().unwrap()));
        assert!(!is_ip_restricted("100.63.255.255".parse().unwrap()));
        assert!(!is_ip_restricted("100.128.0.1".parse().unwrap()));

        // Unspecified & Broadcast
        assert!(is_ip_restricted("0.0.0.0".parse().unwrap()));
        assert!(is_ip_restricted("255.255.255.255".parse().unwrap()));

        // Multicast
        assert!(is_ip_restricted("224.0.0.1".parse().unwrap()));
        assert!(is_ip_restricted("239.255.255.250".parse().unwrap()));

        // Future reserved: 240.0.0.0/4
        assert!(is_ip_restricted("240.0.0.1".parse().unwrap()));
        assert!(is_ip_restricted("250.1.2.3".parse().unwrap()));

        // Testnets
        assert!(is_ip_restricted("192.0.2.1".parse().unwrap()));
        assert!(is_ip_restricted("198.51.100.1".parse().unwrap()));
        assert!(is_ip_restricted("203.0.113.1".parse().unwrap()));
    }

    #[test]
    fn test_rejects_restricted_hostnames() {
        assert!(is_host_restricted("localhost"));
        assert!(is_host_restricted("LocalHost"));
        assert!(is_host_restricted("foo.localhost"));
        assert!(is_host_restricted("my-service.local"));
        assert!(is_host_restricted("database.internal"));
        assert!(is_host_restricted("metadata.google.internal"));
        assert!(is_host_restricted("instance-data"));

        assert!(validate_destination_url_str("http://localhost:3000/").is_err());
        assert!(validate_destination_url_str("http://metadata.google.internal/computeMetadata/v1/").is_err());
    }

    #[test]
    fn test_rejects_dangerous_schemes() {
        assert!(validate_destination_url_str("file:///etc/passwd").is_err());
        assert!(validate_destination_url_str("file://C:/Windows/win.ini").is_err());
        assert!(validate_destination_url_str("gopher://127.0.0.1:70/").is_err());
        assert!(validate_destination_url_str("ftp://example.com/file.txt").is_err());
        assert!(validate_destination_url_str("javascript:alert(1)").is_err());
        assert!(validate_destination_url_str("data:text/html;base64,PHNjcmlwdD4=").is_err());
    }

    #[test]
    fn test_allows_valid_public_ip_and_socket() {
        let public_v4: IpAddr = "93.184.216.34".parse().unwrap();
        assert!(!is_ip_restricted(public_v4));

        let res = validate_destination_url_str("http://93.184.216.34:80/path");
        assert!(res.is_ok());
        let addr = res.unwrap();
        assert_eq!(addr.ip(), public_v4);
        assert_eq!(addr.port(), 80);

        assert!(validate_socket_addr(&addr).is_ok());
    }

    #[test]
    fn test_validate_socket_addr_rejects_private() {
        let bad_sock = SocketAddr::new("127.0.0.1".parse().unwrap(), 80);
        assert!(validate_socket_addr(&bad_sock).is_err());

        let meta_sock = SocketAddr::new("169.254.169.254".parse().unwrap(), 80);
        assert!(validate_socket_addr(&meta_sock).is_err());
    }
}
