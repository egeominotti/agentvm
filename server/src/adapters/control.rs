//! Length-framed JSON control over the helper's existing Unix-to-vsock bridge.
use crate::domain::guest_control::FRAME_LIMIT;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const LEGACY: &str = "this VM runtime does not support structured control; save and close it, then launch a new VM";

pub async fn request(socket: &Path, op: &str, args: Value) -> Result<Value, String> {
    tokio::time::timeout(Duration::from_secs(35), async {
        let capabilities = exchange(socket, "capabilities", json!({})).await.map_err(|_| LEGACY.to_owned())?;
        if capabilities["version"] != 1 {
            return Err(LEGACY.to_owned());
        }
        exchange(socket, op, args).await
    })
    .await
    .map_err(|_| "guest control timed out (legacy VM runtimes require a new VM)".to_owned())?
}

async fn exchange(socket: &Path, op: &str, args: Value) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&json!({"op":op,"args":args})).map_err(|e| e.to_string())?;
    if bytes.len() > FRAME_LIMIT {
        return Err("control frame exceeds 16 MiB".into());
    }
    let mut stream = UnixStream::connect(socket).await.map_err(|e| e.to_string())?;
    stream.write_all(b"{\"control\":true}\n").await.map_err(|e| e.to_string())?;
    stream.write_u32(bytes.len() as u32).await.map_err(|e| e.to_string())?;
    stream.write_all(&bytes).await.map_err(|e| e.to_string())?;
    let length = stream.read_u32().await.map_err(|e| e.to_string())? as usize;
    if length > FRAME_LIMIT {
        return Err("guest response exceeds 16 MiB".into());
    }
    let mut reply = vec![0; length];
    stream.read_exact(&mut reply).await.map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_slice(&reply).map_err(|_| "invalid guest response".to_owned())?;
    if value["ok"] == true {
        Ok(value["result"].clone())
    } else {
        Err(value["error"].as_str().unwrap_or("guest control failed").to_owned())
    }
}
