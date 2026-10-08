//! S3 client on top of `curl --aws-sigv4`: works with AWS S3, Cloudflare R2, Hetzner Object Storage,
//! MinIO, RustFS and other S3-compatible services. Credentials reach curl on stdin, never argv.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::domain::s3::S3Config;
use crate::secret::Secret;

#[derive(Debug, thiserror::Error)]
pub enum S3Error {
    #[error("could not run curl: {0}")]
    Spawn(std::io::Error),
    #[error("S3 answered {status} for {what}{detail}")]
    Http { status: u16, what: String, detail: String },
    #[error("unexpected answer from S3: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Object {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
}

pub struct S3Client {
    cfg: S3Config,
    secret: Secret,
}

impl S3Client {
    pub fn new(cfg: S3Config, secret: Secret) -> Self {
        S3Client { cfg, secret }
    }

    pub fn config(&self) -> &S3Config {
        &self.cfg
    }

    /// Runs curl with SigV4 signing; returns the body on 2xx.
    fn curl(&self, what: &str, args: &[&std::ffi::OsStr]) -> Result<Vec<u8>, S3Error> {
        let mut child = Command::new("curl")
            .args(["-sS", "-K", "-", "--aws-sigv4"])
            .arg(format!("aws:amz:{}:s3", self.cfg.region))
            .args(["-w", "\n%{http_code}"])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(S3Error::Spawn)?;
        {
            let mut stdin = child.stdin.take().expect("stdin is piped");
            let line = format!("user = \"{}:{}\"\n", self.cfg.access_key, self.secret.expose());
            stdin.write_all(line.as_bytes()).map_err(S3Error::Spawn)?;
        }
        let out = child.wait_with_output().map_err(S3Error::Spawn)?;
        let mut body = out.stdout;
        let split = body.iter().rposition(|&b| b == b'\n').unwrap_or(0);
        let status: u16 = String::from_utf8_lossy(&body[split..]).trim().parse().unwrap_or(0);
        body.truncate(split);
        if (200..300).contains(&status) {
            return Ok(body);
        }
        let text = String::from_utf8_lossy(&body);
        let code = text.split("<Code>").nth(1).and_then(|r| r.split("</Code>").next()).map(|c| format!(": {c}"));
        let detail = code.unwrap_or_else(|| {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            if err.is_empty() { String::new() } else { format!(": {err}") }
        });
        Err(S3Error::Http { status, what: what.to_owned(), detail })
    }

    /// Creates the bucket if it does not exist yet (a no-op when it does and is ours).
    pub fn ensure_bucket(&self) -> Result<(), S3Error> {
        if self.curl("the bucket", &["-I".as_ref(), self.cfg.bucket_url().as_ref()]).is_ok() {
            return Ok(());
        }
        self.curl("creating the bucket", &["-X".as_ref(), "PUT".as_ref(), self.cfg.bucket_url().as_ref()]).map(drop)
    }

    pub fn put_file(&self, key: &str, file: &Path) -> Result<(), S3Error> {
        self.curl(key, &["-T".as_ref(), file.as_os_str(), self.cfg.object_url(key).as_ref()]).map(drop)
    }

    pub fn put_bytes(&self, key: &str, bytes: &[u8]) -> Result<(), S3Error> {
        let tmp = std::env::temp_dir().join(format!("agentvm-s3-{}-{}", std::process::id(), key.replace('/', "_")));
        std::fs::write(&tmp, bytes).map_err(S3Error::Spawn)?;
        let result = self.put_file(key, &tmp);
        let _ = std::fs::remove_file(&tmp);
        result
    }

    pub fn get_file(&self, key: &str, file: &Path) -> Result<(), S3Error> {
        let tmp = file.with_extension("part");
        let result = self.curl(key, &["-o".as_ref(), tmp.as_os_str(), self.cfg.object_url(key).as_ref()]);
        match result {
            Ok(_) => std::fs::rename(&tmp, file).map_err(S3Error::Spawn),
            Err(e) => {
                // With -o the error body went to the file: read the S3 error code from there.
                let body = std::fs::read_to_string(&tmp).unwrap_or_default();
                let _ = std::fs::remove_file(&tmp);
                Err(match e {
                    S3Error::Http { status, what, .. } => {
                        let code = body.split("<Code>").nth(1).and_then(|r| r.split("</Code>").next()).map(|c| format!(": {c}")).unwrap_or_default();
                        S3Error::Http { status, what, detail: code }
                    }
                    other => other,
                })
            }
        }
    }

    pub fn get_bytes(&self, key: &str) -> Result<Vec<u8>, S3Error> {
        self.curl(key, &[self.cfg.object_url(key).as_ref()])
    }

    pub fn delete(&self, key: &str) -> Result<(), S3Error> {
        self.curl(key, &["-X".as_ref(), "DELETE".as_ref(), self.cfg.object_url(key).as_ref()]).map(drop)
    }

    /// Objects whose key starts with `prefix` (ListObjectsV2, all pages).
    pub fn list(&self, prefix: &str) -> Result<Vec<S3Object>, S3Error> {
        let mut all = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut url = format!("{}?list-type=2&prefix={}", self.cfg.bucket_url(), encode(prefix));
            if let Some(t) = &token {
                url.push_str(&format!("&continuation-token={}", encode(t)));
            }
            let body = String::from_utf8(self.curl("the bucket listing", &[url.as_ref()])?).map_err(|e| S3Error::Parse(e.to_string()))?;
            for item in body.split("<Contents>").skip(1) {
                let field = |name: &str| item.split(&format!("<{name}>")).nth(1).and_then(|r| r.split(&format!("</{name}>")).next()).unwrap_or_default().to_owned();
                all.push(S3Object { key: xml_unescape(&field("Key")), size: field("Size").parse().unwrap_or(0), last_modified: field("LastModified") });
            }
            let truncated = body.contains("<IsTruncated>true</IsTruncated>");
            token = body.split("<NextContinuationToken>").nth(1).and_then(|r| r.split("</NextContinuationToken>").next()).map(str::to_owned);
            if !truncated || token.is_none() {
                return Ok(all);
            }
        }
    }
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}
