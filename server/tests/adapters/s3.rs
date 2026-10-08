//! The S3 client: a real dev server, slow servers and hostile secrets.

use std::time::Duration;

use agentvm::secret::Secret;

fn dev_s3() -> agentvm::adapters::s3::S3Client {
    use agentvm::domain::s3::S3Config;
    let cfg = S3Config {
        endpoint: "http://127.0.0.1:9100".into(),
        region: "us-east-1".into(),
        bucket: "agentvm-backups".into(),
        prefix: format!("test-{}", std::process::id()),
        access_key: "agentvm".into(),
        path_style: true,
    };
    agentvm::adapters::s3::S3Client::new(cfg, Secret::new("agentvm-local-secret".into()))
}

#[test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
fn s3_client_round_trips_objects_on_a_real_server() {
    let s3 = dev_s3();
    s3.ensure_bucket().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let up = tmp.path().join("up.bin");
    std::fs::write(&up, vec![7u8; 2 << 20]).unwrap();
    assert_eq!(s3.put_file(&s3.config().key("a/b.bin"), &up).unwrap(), 1);
    s3.put_bytes(&s3.config().key("a/b.json"), b"{\"ok\":true}").unwrap();
    let listed = s3.list(&s3.config().key("a/")).unwrap();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed.iter().any(|o| o.key.ends_with("b.bin") && o.size == 2 << 20), "{listed:?}");
    let down = tmp.path().join("down.bin");
    s3.get_file(&s3.config().key("a/b.bin"), &down).unwrap();
    assert_eq!(std::fs::read(&down).unwrap(), std::fs::read(&up).unwrap());
    assert_eq!(s3.get_bytes(&s3.config().key("a/b.json")).unwrap(), b"{\"ok\":true}");
    s3.delete(&s3.config().key("a/b.bin")).unwrap();
    s3.delete(&s3.config().key("a/b.json")).unwrap();
    assert!(s3.list(&s3.config().key("a/")).unwrap().is_empty());
}

#[test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
fn s3_client_reports_bad_credentials() {
    use agentvm::domain::s3::S3Config;
    let good = dev_s3();
    let bad =
        agentvm::adapters::s3::S3Client::new(S3Config { ..good.config().clone() }, Secret::new("wrong-secret".into()));
    assert!(bad.list("x/").is_err());
}

#[test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
fn s3_large_files_go_up_in_parts() {
    let s3 = dev_s3().with_part_size(5 << 20);
    s3.ensure_bucket().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let up = tmp.path().join("big.bin");
    // 12 MiB of varied bytes: three parts (5 + 5 + 2).
    let data: Vec<u8> = (0..12u32 << 20).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(&up, &data).unwrap();
    let key = s3.config().key("multi/big.bin");
    let parts = s3.put_file(&key, &up).unwrap();
    assert_eq!(parts, 3);
    let down = tmp.path().join("down.bin");
    s3.get_file(&key, &down).unwrap();
    assert_eq!(std::fs::read(&down).unwrap(), data);
    s3.delete(&key).unwrap();
}

fn s3_to(endpoint: String) -> agentvm::adapters::s3::S3Client {
    let cfg = agentvm::domain::s3::S3Config {
        endpoint,
        region: "us-east-1".into(),
        bucket: "agentvm-backups".into(),
        prefix: "agentvm".into(),
        access_key: "k".into(),
        path_style: true,
    };
    agentvm::adapters::s3::S3Client::new(cfg, Secret::new("s".into()))
}

/// An endpoint that accepts connections and then never answers must not hang a backup forever.
#[test]
fn s3_calls_give_up_on_a_server_that_stops_answering() {
    let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = silent.local_addr().unwrap().port();
    let _keep = std::thread::spawn(move || {
        let held: Vec<_> = silent.incoming().take(16).collect();
        std::thread::sleep(Duration::from_secs(120));
        drop(held);
    });
    let s3 = s3_to(format!("http://127.0.0.1:{port}")).with_stall_timeout(2);
    let t0 = std::time::Instant::now();
    assert!(s3.put_bytes("agentvm/x", b"x").is_err());
    assert!(t0.elapsed() < Duration::from_secs(20), "took {:?}", t0.elapsed());
}

/// The secret goes into curl's config: quotes and backslashes must not inject curl options.
#[test]
fn an_s3_secret_cannot_inject_curl_options() {
    let tmp = tempfile::tempdir().unwrap();
    let stolen = tmp.path().join("stolen");
    let cfg = agentvm::domain::s3::S3Config {
        endpoint: "http://127.0.0.1:9".into(),
        region: "us-east-1".into(),
        bucket: "agentvm-backups".into(),
        prefix: "agentvm".into(),
        access_key: "k".into(),
        path_style: true,
    };
    // A valid config once injected: curl writes its trace even when the connection fails.
    let evil = format!("s\"\ntrace-ascii = \"{}\"\nreferer = \"", stolen.display());
    let s3 = agentvm::adapters::s3::S3Client::new(cfg, Secret::new(evil)).with_stall_timeout(2);
    let _ = s3.put_bytes("agentvm/x", b"x");
    assert!(!stolen.exists(), "the secret added a curl option");
}

/// A server that promises 1000 bytes, sends 10 and hangs up: the download fails, and no
/// truncated file is left in place of the backup (it used to be accepted on the 200 status).
#[test]
fn a_truncated_download_is_an_error_not_a_file() {
    use agentvm::domain::s3::S3Config;
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(4).flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n0123456789");
        }
    });
    let cfg = S3Config {
        endpoint: format!("http://127.0.0.1:{port}"),
        region: "us-east-1".into(),
        bucket: "agentvm-backups".into(),
        prefix: "t".into(),
        access_key: "k".into(),
        path_style: true,
    };
    let s3 = agentvm::adapters::s3::S3Client::new(cfg, Secret::new("s".into())).with_stall_timeout(2);
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("backup.tar.zst");
    assert!(s3.get_file("t/backup.tar.zst", &file).is_err());
    assert!(!file.exists(), "a truncated backup was put in place");
}
