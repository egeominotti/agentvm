//! Byte-aware line splitting and bounded checkpoint anchors.
use super::MAX_LINE;

#[derive(Default)]
pub(super) struct Lines {
    pending: Vec<u8>,
    skipping: Option<usize>,
    trailer: Vec<u8>,
}

impl Lines {
    pub(super) fn with_trailer(trailer: Vec<u8>) -> Self {
        Self { trailer, ..Self::default() }
    }

    pub(super) fn anchor(&self) -> String {
        blake3::hash(&self.trailer).to_hex().to_string()
    }

    pub(super) fn push(&mut self, bytes: &[u8], start: u64) -> Vec<(String, u64, String)> {
        let mut out = Vec::new();
        let mut offset = start;
        for part in bytes.split_inclusive(|&b| b == b'\n') {
            offset += part.len() as u64;
            if part.len() >= 64 {
                self.trailer = part[part.len() - 64..].to_vec();
            } else {
                self.trailer.extend_from_slice(part);
                self.trailer.drain(..self.trailer.len().saturating_sub(64));
            }
            let (body, ends) = match part.strip_suffix(b"\n") {
                Some(body) => (body, true),
                None => (part, false),
            };
            if let Some(skipped) = self.skipping.as_mut() {
                *skipped += part.len();
            } else if self.pending.len() + body.len() > MAX_LINE {
                self.skipping = Some(self.pending.len() + part.len());
                self.pending.clear();
            } else {
                self.pending.extend_from_slice(body);
            }
            if ends {
                let line = match self.skipping.take() {
                    Some(n) => format!("(agentvm skipped a line of {} MB)", n >> 20),
                    None => String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned(),
                };
                out.push((line, offset, self.anchor()));
            }
        }
        out
    }

    pub(super) fn rest(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned())
    }
}
