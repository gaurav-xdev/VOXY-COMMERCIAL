//! Integration tests for Milestone M4 (R4 & A4):
//! SSRF & DNS Rebinding socket defense and Archive extraction safety with rollback.

use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use voxy_security::{
    is_host_restricted, is_ip_restricted, safe_extract_tar, safe_extract_zip,
    validate_archive_path, validate_destination_url_str, validate_socket_addr,
    ArchiveSecurityLimits,
};
use zip::write::SimpleFileOptions;

// =========================================================================
// 1. SSRF & Rebinding Defense Tests (A4 Acceptance Criteria)
// =========================================================================

#[test]
fn test_ssrf_rejects_loopback_destinations() {
    let loopbacks = [
        "127.0.0.1",
        "127.0.0.2",
        "127.100.200.1",
        "127.255.255.254",
        "127.255.255.255",
    ];

    for ip_str in loopbacks {
        let ip: IpAddr = ip_str.parse().unwrap();
        assert!(
            is_ip_restricted(ip),
            "Loopback IP {} must be restricted",
            ip_str
        );
        assert!(validate_socket_addr(&SocketAddr::new(ip, 80)).is_err());
        assert!(validate_destination_url_str(&format!("http://{ip_str}:8080/admin")).is_err());
    }

    // IPv6 Loopback
    let v6_loopback: IpAddr = "::1".parse().unwrap();
    assert!(is_ip_restricted(v6_loopback));
    assert!(validate_socket_addr(&SocketAddr::new(v6_loopback, 8080)).is_err());
    assert!(validate_destination_url_str("http://[::1]:8080/status").is_err());

    // IPv4-mapped IPv6 loopback
    let v6_mapped: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
    assert!(is_ip_restricted(v6_mapped));
    assert!(validate_destination_url_str("http://[::ffff:127.0.0.1]:8080/").is_err());
}

#[test]
fn test_ssrf_rejects_cloud_metadata_destinations() {
    let metadata_targets = [
        "169.254.169.254", // AWS/GCP/Azure IMDS
        "169.254.170.2",   // AWS ECS task metadata
        "100.100.100.200", // Alibaba Cloud metadata
    ];

    for ip_str in metadata_targets {
        let ip: IpAddr = ip_str.parse().unwrap();
        assert!(
            is_ip_restricted(ip),
            "Metadata IP {} must be restricted",
            ip_str
        );
        assert!(validate_socket_addr(&SocketAddr::new(ip, 80)).is_err());
        assert!(
            validate_destination_url_str(&format!("http://{ip_str}/latest/meta-data/")).is_err()
        );
    }

    // AWS IPv6 IMDS
    let v6_meta: IpAddr = "fd00:ec2::254".parse().unwrap();
    assert!(is_ip_restricted(v6_meta));
    assert!(validate_destination_url_str("http://[fd00:ec2::254]/latest/meta-data").is_err());

    // Hostnames
    assert!(is_host_restricted("metadata.google.internal"));
    assert!(is_host_restricted("instance-data"));
    assert!(
        validate_destination_url_str("http://metadata.google.internal/computeMetadata/v1/")
            .is_err()
    );
    assert!(validate_destination_url_str("http://instance-data/latest/meta-data").is_err());
}

#[test]
fn test_ssrf_rejects_all_rfc1918_private_subnets() {
    let private_ips = [
        // 10.0.0.0/8
        "10.0.0.1",
        "10.10.10.10",
        "10.254.254.254",
        "10.255.255.255",
        // 172.16.0.0/12
        "172.16.0.1",
        "172.20.5.10",
        "172.31.255.254",
        "172.31.255.255",
        // 192.168.0.0/16
        "192.168.0.1",
        "192.168.1.1",
        "192.168.100.50",
        "192.168.255.255",
    ];

    for ip_str in private_ips {
        let ip: IpAddr = ip_str.parse().unwrap();
        assert!(
            is_ip_restricted(ip),
            "RFC1918 IP {} must be restricted",
            ip_str
        );
        assert!(validate_socket_addr(&SocketAddr::new(ip, 443)).is_err());
        assert!(validate_destination_url_str(&format!("http://{ip_str}:8080/internal")).is_err());
    }

    // Verify subnets right outside 172.16.0.0/12 are not marked as RFC 1918
    assert!(!is_ip_restricted("172.15.255.255".parse().unwrap()));
    assert!(!is_ip_restricted("172.32.0.1".parse().unwrap()));
}

#[test]
fn test_ssrf_rejects_link_local_and_reserved_subnets() {
    let link_locals = ["169.254.0.1", "169.254.10.20", "169.254.255.254"];

    for ip_str in link_locals {
        let ip: IpAddr = ip_str.parse().unwrap();
        assert!(is_ip_restricted(ip));
        assert!(validate_destination_url_str(&format!("http://{ip_str}/")).is_err());
    }

    // IPv6 link local fe80::/10
    let fe80_ip: IpAddr = "fe80::1ff:fe00:1".parse().unwrap();
    assert!(is_ip_restricted(fe80_ip));
    assert!(validate_destination_url_str("http://[fe80::1ff:fe00:1]/").is_err());

    // CGNAT 100.64.0.0/10
    assert!(is_ip_restricted("100.64.0.1".parse().unwrap()));
    assert!(is_ip_restricted("100.127.255.254".parse().unwrap()));

    // Broadcast, unspecified, multicast
    assert!(is_ip_restricted("0.0.0.0".parse().unwrap()));
    assert!(is_ip_restricted("255.255.255.255".parse().unwrap()));
    assert!(is_ip_restricted("224.0.0.1".parse().unwrap()));
    assert!(is_ip_restricted("239.255.255.250".parse().unwrap()));
    assert!(is_ip_restricted("::".parse().unwrap()));
}

#[test]
fn test_ssrf_allows_public_endpoints() {
    let public_ip: IpAddr = "93.184.216.34".parse().unwrap();
    assert!(!is_ip_restricted(public_ip));

    let addr = validate_destination_url_str("https://93.184.216.34:443/test").unwrap();
    assert_eq!(addr.ip(), public_ip);
    assert_eq!(addr.port(), 443);
    assert!(validate_socket_addr(&addr).is_ok());
}

// =========================================================================
// 2. Archive Extraction Safety Tests (A4 Acceptance Criteria)
// =========================================================================

#[test]
fn test_archive_path_validation_blocks_all_traversal_attempts() {
    let limits = ArchiveSecurityLimits::default();

    let malicious_paths = [
        "../escaped.txt",
        "../../etc/shadow",
        "foo/../../bar.txt",
        r"foo\..\evil.txt",
        r"..\..\evil.txt",
        "/etc/passwd",
        r"C:\Windows\System32\cmd.exe",
        "D:/payload.exe",
        "file.txt:hidden_stream",
        "test.txt::$DATA",
        r"\\server\share\file.txt",
        "//10.0.0.1/share/evil.exe",
        "evil\0file.txt",
    ];

    for path in malicious_paths {
        assert!(
            validate_archive_path(path, &limits).is_err(),
            "Path '{}' must be rejected",
            path
        );
    }
}

#[test]
fn test_zip_extraction_clean_rollback_on_traversal() {
    let temp = tempfile::tempdir().unwrap();
    let zip_file_path = temp.path().join("traversal.zip");
    let extract_dir = temp.path().join("dest");

    // Build zip with 2 files: first benign, second malicious traversal
    let file = std::fs::File::create(&zip_file_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("benign_file.txt", options).unwrap();
    zip.write_all(b"Benign file content").unwrap();

    zip.start_file("../outside.txt", options).unwrap();
    zip.write_all(b"Malicious payload").unwrap();
    zip.finish().unwrap();

    let limits = ArchiveSecurityLimits::default();
    let reader = std::fs::File::open(&zip_file_path).unwrap();
    let result = safe_extract_zip(reader, &extract_dir, &limits);

    assert!(result.is_err(), "Zip with traversal must fail");

    // Clean rollback verification: benign_file.txt must NOT exist!
    assert!(
        !extract_dir.join("benign_file.txt").exists(),
        "Rollback failed: benign_file.txt was left behind after extraction failure"
    );
    assert!(!temp.path().join("outside.txt").exists());
}

#[test]
fn test_zip_extraction_clean_rollback_on_entry_quota() {
    let temp = tempfile::tempdir().unwrap();
    let zip_file_path = temp.path().join("quota.zip");
    let extract_dir = temp.path().join("dest");

    let file = std::fs::File::create(&zip_file_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("first.txt", options).unwrap();
    zip.write_all(b"Valid small file").unwrap();

    zip.start_file("second_oversized.bin", options).unwrap();
    zip.write_all(&vec![1u8; 15_000]).unwrap();
    zip.finish().unwrap();

    let limits = ArchiveSecurityLimits {
        max_entry_uncompressed_bytes: 1000, // 1KB limit
        ..Default::default()
    };

    let reader = std::fs::File::open(&zip_file_path).unwrap();
    let result = safe_extract_zip(reader, &extract_dir, &limits);

    assert!(result.is_err());
    assert!(
        !extract_dir.join("first.txt").exists(),
        "Rollback failed for first.txt"
    );
    assert!(!extract_dir.join("second_oversized.bin").exists());
}

#[test]
fn test_zip_extraction_clean_rollback_on_total_quota() {
    let temp = tempfile::tempdir().unwrap();
    let zip_file_path = temp.path().join("total_quota.zip");
    let extract_dir = temp.path().join("dest");

    let file = std::fs::File::create(&zip_file_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for i in 0..5 {
        zip.start_file(format!("part_{i}.bin"), options).unwrap();
        zip.write_all(&vec![2u8; 2000]).unwrap();
    }
    zip.finish().unwrap();

    let limits = ArchiveSecurityLimits {
        max_entry_uncompressed_bytes: 5000,
        max_total_uncompressed_bytes: 4500, // Will fail after 2 files
        ..Default::default()
    };

    let reader = std::fs::File::open(&zip_file_path).unwrap();
    let result = safe_extract_zip(reader, &extract_dir, &limits);

    assert!(result.is_err());
    for i in 0..5 {
        assert!(
            !extract_dir.join(format!("part_{i}.bin")).exists(),
            "Rollback failed: part_{i}.bin exists"
        );
    }
}

#[test]
fn test_zip_extraction_clean_rollback_on_compression_bomb() {
    let temp = tempfile::tempdir().unwrap();
    let zip_file_path = temp.path().join("bomb.zip");
    let extract_dir = temp.path().join("dest");

    let file = std::fs::File::create(&zip_file_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file(
        "good.txt",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(b"good").unwrap();

    zip.start_file("bomb.bin", options).unwrap();
    zip.write_all(&vec![0u8; 100_000]).unwrap(); // Compresses down to tiny size
    zip.finish().unwrap();

    let limits = ArchiveSecurityLimits {
        max_compression_ratio: 5.0, // Very strict ratio
        ..Default::default()
    };

    let reader = std::fs::File::open(&zip_file_path).unwrap();
    let result = safe_extract_zip(reader, &extract_dir, &limits);

    assert!(result.is_err());
    assert!(
        !extract_dir.join("good.txt").exists(),
        "good.txt must be rolled back"
    );
    assert!(!extract_dir.join("bomb.bin").exists());
}

#[test]
fn test_tar_extraction_clean_rollback_on_traversal() {
    let temp = tempfile::tempdir().unwrap();
    let extract_dir = temp.path().join("tar_dest");

    let mut tar_builder = tar::Builder::new(Vec::new());

    // File 1: Benign
    let mut header1 = tar::Header::new_gnu();
    header1.set_path("benign.txt").unwrap();
    header1.set_size(6);
    header1.set_mode(0o644);
    header1.set_cksum();
    tar_builder.append(&header1, &b"benign"[..]).unwrap();

    // File 2: Traversal
    let mut header2 = tar::Header::new_gnu();
    let evil_path = b"../escaped_tar.txt\0";
    header2.as_mut_bytes()[..evil_path.len()].copy_from_slice(evil_path);
    header2.set_size(5);
    header2.set_mode(0o644);
    header2.set_cksum();
    tar_builder.append(&header2, &b"evil!"[..]).unwrap();

    let tar_bytes = tar_builder.into_inner().unwrap();
    let limits = ArchiveSecurityLimits::default();

    let result = safe_extract_tar(&tar_bytes[..], &extract_dir, &limits);
    assert!(result.is_err());

    // Clean rollback: benign.txt must be deleted!
    assert!(
        !extract_dir.join("benign.txt").exists(),
        "Rollback failed: benign.txt left behind"
    );
    assert!(!temp.path().join("escaped_tar.txt").exists());
}

#[test]
fn test_tar_extraction_rejects_symlink_escape() {
    let temp = tempfile::tempdir().unwrap();
    let extract_dir = temp.path().join("tar_dest");

    let mut tar_builder = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_path("symlink_escape").unwrap();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_link_name("/etc/passwd").unwrap();
    header.set_size(0);
    header.set_cksum();
    tar_builder.append(&header, std::io::empty()).unwrap();

    let tar_bytes = tar_builder.into_inner().unwrap();
    let limits = ArchiveSecurityLimits::default();

    let result = safe_extract_tar(&tar_bytes[..], &extract_dir, &limits);
    assert!(result.is_err());
    assert!(!extract_dir.join("symlink_escape").exists());
}
