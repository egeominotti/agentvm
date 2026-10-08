//! Large uploads in parts: CreateMultipartUpload, UploadPart × n, CompleteMultipartUpload.

use std::path::Path;

use super::{S3Client, S3Error, between, encode};

impl S3Client {
    /// CreateMultipartUpload → UploadPart × n → CompleteMultipartUpload; aborted on any error.
    pub(super) fn put_multipart(&self, key: &str, file: &Path, size: u64) -> Result<usize, S3Error> {
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
}
