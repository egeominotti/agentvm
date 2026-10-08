//! Valore segreto: non stampabile, leggibile solo esplicitamente.

use std::fmt;

pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Secret(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Secret {
    /// Sostituisce ogni occorrenza del valore con `[REDACTED]`.
    pub fn redact(&self, text: &str) -> String {
        if self.0.is_empty() { text.to_owned() } else { text.replace(&self.0, "[REDACTED]") }
    }
}
