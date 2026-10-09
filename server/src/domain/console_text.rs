//! A VM's console as text a person can read: a terminal's control sequences (cursor moves,
//! colors, title changes, size queries) removed, line ends made plain.

/// `raw` without escape sequences, carriage returns or other control characters (tabs and
/// newlines stay). A sequence cut off at the end is dropped.
pub fn readable(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.next() {
                // CSI: parameters, then one final byte in @..~.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC, DCS and the like: up to BEL or the string terminator ESC \.
                Some(']' | 'P' | '_' | '^') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' || (c == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                // Two-character sequences (ESC 7, ESC =, …).
                _ => {}
            },
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}
