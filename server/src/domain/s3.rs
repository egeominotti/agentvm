//! S3-compatible storage settings (AWS S3, Cloudflare R2, Hetzner Object Storage, MinIO, RustFS…).
//! The secret key is not here: it lives in the Keychain.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct S3Config {
    /// `https://s3.eu-central-1.amazonaws.com`, `https://<account>.r2.cloudflarestorage.com`,
    /// `https://fsn1.your-objectstorage.com`, `http://127.0.0.1:9100`…
    pub endpoint: String,
    /// Signing region: `us-east-1`, `auto` (R2), `fsn1` (Hetzner)…
    pub region: String,
    pub bucket: String,
    /// Folder inside the bucket for agentvm's backups.
    pub prefix: String,
    pub access_key: String,
    /// `endpoint/bucket/key` instead of `bucket.endpoint/key`. Needed by MinIO-like servers.
    pub path_style: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum S3ConfigError {
    #[error("the endpoint must start with https:// or http://")]
    Endpoint,
    #[error("the region is required (use auto for Cloudflare R2)")]
    Region,
    #[error("bucket names use 3–63 lowercase letters, digits, dots and dashes")]
    Bucket,
    #[error("the prefix may only use letters, digits, dashes, dots, underscores and slashes")]
    Prefix,
    #[error("the access key looks wrong")]
    AccessKey,
}

impl S3Config {
    pub fn validate(&self) -> Result<(), S3ConfigError> {
        let host = self.endpoint.strip_prefix("https://").or_else(|| self.endpoint.strip_prefix("http://"));
        if !host.is_some_and(|h| !h.trim_end_matches('/').is_empty() && !h.contains(char::is_whitespace)) {
            return Err(S3ConfigError::Endpoint);
        }
        if self.region.trim().is_empty() || self.region.contains(char::is_whitespace) {
            return Err(S3ConfigError::Region);
        }
        let b = &self.bucket;
        if !(3..=63).contains(&b.len())
            || !b.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
        {
            return Err(S3ConfigError::Bucket);
        }
        if !self.prefix.bytes().all(|c| c.is_ascii_alphanumeric() || b"-._/".contains(&c)) || self.prefix.contains("..")
        {
            return Err(S3ConfigError::Prefix);
        }
        if self.access_key.is_empty()
            || !self.access_key.bytes().all(|c| c.is_ascii_graphic())
            || self.access_key.contains('"')
        {
            return Err(S3ConfigError::AccessKey);
        }
        Ok(())
    }

    fn endpoint(&self) -> &str {
        self.endpoint.trim_end_matches('/')
    }

    pub fn bucket_url(&self) -> String {
        if self.path_style {
            format!("{}/{}", self.endpoint(), self.bucket)
        } else {
            let (scheme, host) = self.endpoint().split_once("://").unwrap_or(("https", self.endpoint()));
            format!("{scheme}://{}.{host}", self.bucket)
        }
    }

    pub fn object_url(&self, key: &str) -> String {
        format!("{}/{}", self.bucket_url(), key)
    }

    /// Object key under the configured prefix.
    pub fn key(&self, name: &str) -> String {
        let prefix = self.prefix.trim_matches('/');
        if prefix.is_empty() { name.to_owned() } else { format!("{prefix}/{name}") }
    }
}
