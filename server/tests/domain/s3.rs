//! S3 configuration and addressing.

use agentvm::domain::s3::S3Config;

fn s3(endpoint: &str, path_style: bool) -> S3Config {
    S3Config {
        endpoint: endpoint.into(),
        region: "auto".into(),
        bucket: "backups".into(),
        prefix: "agentvm".into(),
        access_key: "AK".into(),
        path_style,
    }
}

#[test]
fn s3_urls_follow_the_addressing_style() {
    assert_eq!(
        s3("https://acc.r2.cloudflarestorage.com", true).object_url("agentvm/x.json"),
        "https://acc.r2.cloudflarestorage.com/backups/agentvm/x.json"
    );
    assert_eq!(
        s3("https://s3.eu-central-1.amazonaws.com/", false).object_url("agentvm/x.json"),
        "https://backups.s3.eu-central-1.amazonaws.com/agentvm/x.json"
    );
    assert_eq!(s3("http://127.0.0.1:9100", true).bucket_url(), "http://127.0.0.1:9100/backups");
    assert_eq!(s3("http://127.0.0.1:9100", true).key("snap-1.tar.zst"), "agentvm/snap-1.tar.zst");
}

#[test]
fn s3_config_is_validated() {
    assert!(s3("https://fsn1.your-objectstorage.com", false).validate().is_ok());
    assert!(s3("ftp://x", true).validate().is_err());
    assert!(S3Config { bucket: "Bad_Bucket".into(), ..s3("https://x.com", true) }.validate().is_err());
    assert!(S3Config { region: "".into(), ..s3("https://x.com", true) }.validate().is_err());
    assert!(S3Config { access_key: "a b".into(), ..s3("https://x.com", true) }.validate().is_err());
}
