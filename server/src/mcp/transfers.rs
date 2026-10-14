//! Explicit host-file artifacts stream through archive/upload APIs, never through tool results.
use super::client::{ApiClient, result};
use super::parameters::{TransferArgs, TransferOp};
use super::segment;
use futures::StreamExt;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

struct PartialFile(std::path::PathBuf, bool);
impl Drop for PartialFile {
    fn drop(&mut self) {
        if !self.1 {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

pub async fn transfer(client: &ApiClient, args: TransferArgs) -> Result<Value, String> {
    let path = std::path::PathBuf::from(&args.local_path);
    if !path.is_absolute() {
        return Err("local_path must be an explicit absolute host file path".into());
    }
    let id = args.id.as_deref().map(segment).transpose()?;
    match args.operation {
        TransferOp::Export => {
            let id = id.ok_or("snapshot id is required for export")?;
            let url = client.origin.join(&format!("/api/snapshots/{id}/export")).map_err(|_| "invalid export route")?;
            let response = client.http.get(url).send().await.map_err(|_| "cannot reach AgentVM")?;
            if !response.status().is_success() {
                return result(response).await;
            }
            let mut file = tokio::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .await
                .map_err(|_| "cannot create export file (existing files are never overwritten)")?;
            let mut partial = PartialFile(path, false);
            let mut stream = response.bytes_stream();
            let mut bytes = 0u64;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| "snapshot export stream failed")?;
                bytes += chunk.len() as u64;
                file.write_all(&chunk).await.map_err(|_| "cannot write export file")?;
            }
            file.sync_all().await.map_err(|_| "cannot finish export file")?;
            partial.1 = true;
            Ok(json!({"local_path":args.local_path,"bytes":bytes}))
        }
        TransferOp::Import | TransferOp::Upload => {
            if !tokio::fs::metadata(&path).await.map_err(|_| "cannot stat host artifact")?.is_file() {
                return Err("local_path must refer to a regular file".into());
            }
            let file = tokio::fs::File::open(&path).await.map_err(|_| "cannot open host artifact file")?;
            if !file.metadata().await.map_err(|_| "cannot stat host artifact")?.is_file() {
                return Err("local_path must refer to a regular file".into());
            }
            let route = if matches!(args.operation, TransferOp::Import) {
                "/api/snapshots/import".into()
            } else {
                format!("/api/tasks/{}/upload", id.ok_or("VM id is required for upload")?)
            };
            let url = client.origin.join(&route).map_err(|_| "invalid transfer route")?;
            let mut request = client.http.post(url).header("content-type", "application/octet-stream");
            if matches!(args.operation, TransferOp::Upload) {
                let name = path.file_name().and_then(|n| n.to_str()).ok_or("invalid upload filename")?;
                let encoded: String =
                    url::form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>().replace('+', "%20");
                request = request.header("x-file-name", encoded);
            }
            let response = request
                .body(reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file)))
                .send()
                .await
                .map_err(|_| "artifact transfer failed")?;
            result(response).await
        }
    }
}
