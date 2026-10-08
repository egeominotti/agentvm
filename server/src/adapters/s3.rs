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
    part_size: u64,
    stall_s: u64,
}

/// Parts of 64 MiB: up to 10,000 parts, so ~640 GB per object; S3 needs at least 5 MiB per part.
const DEFAULT_PART_SIZE: u64 = 64 << 20;
const MIN_PART_SIZE: u64 = 5 << 20;

impl S3Client {
    pub fn new(cfg: S3Config, secret: Secret) -> Self {
        S3Client { cfg, secret, part_size: DEFAULT_PART_SIZE, stall_s: 60 }
    }

    /// Gives up on a transfer that makes no progress for this long (default 60 s).
    pub fn with_stall_timeout(mut self, secs: u64) -> Self {
        self.stall_s = secs;
        self
    }

    /// Files larger than one part are uploaded in parts of this size (at least 5 MiB).
    pub fn with_part_size(mut self, bytes: u64) -> Self {
        self.part_size = bytes.max(MIN_PART_SIZE);
        self
    }

    pub fn config(&self) -> &S3Config {
        &self.cfg
    }

    /// Runs curl with SigV4 signing; returns the body on 2xx.
    fn curl(&self, what: &str, args: &[&std::ffi::OsStr]) -> Result<Vec<u8>, S3Error> {
        let stall = self.stall_s.to_string();
        let mut child = Command::new("curl")
            // A dead or blackholed endpoint fails instead of hanging; transient errors (timeouts,
            // 429, 5xx) are retried, so one hiccup does not abort a 10,000-part upload.
            .args(["--connect-timeout", &stall, "--speed-limit", "1", "--speed-time", &stall, "--max-time", "3600"])
            .args(["--retry", "3", "--retry-delay", "1", "--retry-connrefused"])
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
            // curl config syntax: inside quotes, only \\ and \" are special; a newline would end the
            // option and start another one, so it is escaped too.
            let quote =
                |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r");
            let line = format!("user = \"{}:{}\"\n", quote(&self.cfg.access_key), quote(self.secret.expose()));
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

    /// Uploads a file; returns the number of parts used (1 = single PUT).
    pub fn put_file(&self, key: &str, file: &Path) -> Result<usize, S3Error> {
        let size = std::fs::metadata(file).map_err(S3Error::Spawn)?.len();
        if size <= self.part_size {
            self.curl(key, &["-T".as_ref(), file.as_os_str(), self.cfg.object_url(key).as_ref()])?;
            return Ok(1);
        }
        self.put_multipart(key, file, size)
    }

    /// CreateMultipartUpload → UploadPart × n → CompleteMultipartUpload; aborted on any error.
    fn put_multipart(&self, key: &str, file: &Path, size: u64) -> Result<usize, S3Error> {
        let url = self.cfg.object_url(key);
        let created = String::from_utf8_lossy(
            &self.curl(key, &["-X".as_ref(), "POST".as_ref(), format!("{url}?uploads").as_ref()])?,
        )
        .into_owned();
        let upload_id =
            between(&created, "UploadId").ok_or_else(|| S3Error::Parse(format!("no UploadId in {created}")))?;
        let result = self.upload_parts(key, &url, &upload_id, file, size);
        if result.is_err() {
            let _ = self.curl(
                key,
                &["-X".as_ref(), "DELETE".as_ref(), format!("{url}?uploadId={}", encode(&upload_id)).as_ref()],
            );
        }
        result
    }

    fn upload_parts(&self, key: &str, url: &str, upload_id: &str, file: &Path, size: u64) -> Result<usize, S3Error> {
        use std::io::{Read, Seek, SeekFrom};
        let io = S3Error::Spawn;
        let part_file = std::env::temp_dir().join(format!(
            "agentvm-part-{}-{}",
            std::process::id(),
            encode(upload_id).replace('%', "")
        ));
        let mut source = std::fs::File::open(file).map_err(io)?;
        let mut etags = Vec::new();
        let mut buf = vec![0u8; self.part_size as usize];
        let mut offset = 0u64;
        let outcome = (|| {
            while offset < size {
                let len = (size - offset).min(self.part_size) as usize;
                source.seek(SeekFrom::Start(offset)).map_err(io)?;
                source.read_exact(&mut buf[..len]).map_err(io)?;
                std::fs::write(&part_file, &buf[..len]).map_err(io)?;
                let number = etags.len() + 1;
                let part_url = format!("{url}?partNumber={number}&uploadId={}", encode(upload_id));
                let out = self.curl(
                    key,
                    &["-D".as_ref(), "-".as_ref(), "-T".as_ref(), part_file.as_os_str(), part_url.as_ref()],
                )?;
                let headers = String::from_utf8_lossy(&out);
                let etag = headers
                    .lines()
                    .find_map(|l| {
                        l.split_once(':')
                            .filter(|(k, _)| k.eq_ignore_ascii_case("etag"))
                            .map(|(_, v)| v.trim().to_owned())
                    })
                    .ok_or_else(|| S3Error::Parse(format!("no ETag for part {number}")))?;
                etags.push(etag);
                offset += len as u64;
            }
            Ok(())
        })();
        let _ = std::fs::remove_file(&part_file);
        outcome?;
        let body: String = etags
            .iter()
            .enumerate()
            .map(|(i, e)| format!("<Part><PartNumber>{}</PartNumber><ETag>{e}</ETag></Part>", i + 1))
            .collect();
        let complete = format!("<CompleteMultipartUpload>{body}</CompleteMultipartUpload>");
        let answer = self.curl(
            key,
            &[
                "-X".as_ref(),
                "POST".as_ref(),
                "-H".as_ref(),
                "Content-Type: application/xml".as_ref(),
                "--data-binary".as_ref(),
                complete.as_str().as_ref(),
                format!("{url}?uploadId={}", encode(upload_id)).as_ref(),
            ],
        )?;
        let text = String::from_utf8_lossy(&answer);
        // S3 can answer 200 and still report an error in the body.
        if text.contains("<Error>") {
            return Err(S3Error::Parse(format!("completing the upload failed: {text}")));
        }
        Ok(etags.len())
    }

    pub fn put_bytes(&self, key: &str, bytes: &[u8]) -> Result<(), S3Error> {
        let tmp = std::env::temp_dir().join(format!("agentvm-s3-{}-{}", std::process::id(), key.replace('/', "_")));
        std::fs::write(&tmp, bytes).map_err(S3Error::Spawn)?;
        let result = self.put_file(key, &tmp).map(drop);
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
                        let code = body
                            .split("<Code>")
                            .nth(1)
                            .and_then(|r| r.split("</Code>").next())
                            .map(|c| format!(": {c}"))
                            .unwrap_or_default();
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
            let body = String::from_utf8(self.curl("the bucket listing", &[url.as_ref()])?)
                .map_err(|e| S3Error::Parse(e.to_string()))?;
            for item in body.split("<Contents>").skip(1) {
                let field = |name: &str| {
                    item.split(&format!("<{name}>"))
                        .nth(1)
                        .and_then(|r| r.split(&format!("</{name}>")).next())
                        .unwrap_or_default()
                        .to_owned()
                };
                all.push(S3Object {
                    key: xml_unescape(&field("Key")),
                    size: field("Size").parse().unwrap_or(0),
                    last_modified: field("LastModified"),
                });
            }
            let truncated = body.contains("<IsTruncated>true</IsTruncated>");
            token = body
                .split("<NextContinuationToken>")
                .nth(1)
                .and_then(|r| r.split("</NextContinuationToken>").next())
                .map(str::to_owned);
            if !truncated || token.is_none() {
                return Ok(all);
            }
        }
    }
}

fn between(text: &str, tag: &str) -> Option<String> {
    Some(text.split(&format!("<{tag}>")).nth(1)?.split(&format!("</{tag}>")).next()?.to_owned())
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
