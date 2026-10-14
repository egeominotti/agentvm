//! Validation shared by structured guest-control consumers.
use serde_json::Value;

pub const FRAME_LIMIT: usize = 16 << 20;
pub const CHUNK_LIMIT: usize = 1 << 20;

pub fn validate(op: &str, args: &Value) -> Result<(), String> {
    let object = args.as_object().ok_or("arguments must be an object")?;
    match op {
        "capabilities" => {}
        "exec.start" => {
            let argv = object.get("argv").and_then(Value::as_array).ok_or("argv must be an array")?;
            if argv.is_empty() || argv.iter().any(|a| a.as_str().is_none_or(|s| s.contains('\0'))) {
                return Err("argv must be a nonempty array of strings without NUL bytes".into());
            }
            bound(args, "timeout_s", 1, 1800)?;
            bound(args, "output_limit", 1, CHUNK_LIMIT as u64)?;
            if let Some(stdin) = object.get("stdin")
                && stdin.as_str().is_none_or(|s| s.len() > CHUNK_LIMIT)
            {
                return Err("stdin must be text at most 1 MiB".into());
            }
        }
        "exec.read" | "exec.cancel" => text(args, "exec_id")?,
        "files.list" | "files.stat" | "files.read" | "files.write" | "files.mkdir" | "files.remove"
        | "files.rename" => {
            text(args, "path")?;
            bound(args, "offset", 0, i64::MAX as u64)?;
            bound(args, "size", 1, CHUNK_LIMIT as u64)?;
            bound(args, "limit", 1, 1000)?;
            if op == "files.rename" {
                text(args, "destination")?;
            }
            if op == "files.write" {
                if args["data"].as_str().is_none() {
                    return Err("data must be base64 text".into());
                }
                if args["data"].as_str().unwrap().len() > CHUNK_LIMIT.div_ceil(3) * 4 {
                    return Err("data exceeds the 1 MiB chunk limit".into());
                }
            }
        }
        "terminal.list" => {}
        "terminal.capture" | "terminal.input" | "terminal.resize" | "terminal.create" => {
            let session = args["session"].as_str().ok_or("session is required")?;
            if !super::terminal_session::is_session(session) {
                return Err("unsupported terminal session".into());
            }
            bound(args, "lines", 1, 2000)?;
            bound(args, "cols", 1, 500)?;
            bound(args, "rows", 1, 500)?;
            if let Some(text) = object.get("text")
                && text.as_str().is_none_or(|s| s.len() > 65536)
            {
                return Err("terminal text exceeds 64 KiB".into());
            }
        }
        _ => return Err("unsupported control operation".into()),
    }
    Ok(())
}

fn text(args: &Value, name: &str) -> Result<(), String> {
    if args[name].as_str().is_none_or(|s| s.is_empty() || s.contains('\0')) {
        return Err(format!("{name} must be a nonempty string without NUL bytes"));
    }
    Ok(())
}

fn bound(args: &Value, name: &str, min: u64, max: u64) -> Result<(), String> {
    if let Some(value) = args.get(name)
        && value.as_u64().is_none_or(|n| n < min || n > max)
    {
        return Err(format!("{name} must be between {min} and {max}"));
    }
    Ok(())
}
