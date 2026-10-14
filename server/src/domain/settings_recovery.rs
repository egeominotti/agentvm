//! Saved settings read back: a valid file as it is; otherwise field by field, what no longer fits
//! (another Mac, an older version, a hand edit) falls back to its default (fitted to this Mac),
//! everything else is kept.

use serde_json::Value;

use super::settings::{HostLimits, Settings};

/// The settings to use and the names of the saved fields that were reset.
pub fn recover(saved: &Value, defaults: &Settings, host: &HostLimits) -> (Settings, Vec<String>) {
    let defaults = defaults.clone().fitted(host);
    // A file that is valid as a whole is kept as it is, whatever the defaults.
    if let Ok(whole) = serde_json::from_value::<Settings>(saved.clone())
        && whole.validate(host).is_ok()
    {
        return (whole, Vec::new());
    }
    let Some(fields) = saved.as_object() else {
        return (defaults, vec!["(the whole file)".to_owned()]);
    };
    let mut current = defaults;
    let mut reset = Vec::new();
    // Selection, templates and endpoint overrides form one dependency group. Recover the
    // largest valid subset together before replaying unrelated fields.
    let linked = ["agent", "custom_cli", "providers"];
    for kept in (0..=3).rev() {
        let found = (0u8..8).rev().filter(|mask| mask.count_ones() == kept).find_map(|mask| {
            let mut candidate = serde_json::to_value(&current).expect("settings serialize");
            for (i, name) in linked.iter().enumerate() {
                if mask & (1 << i) != 0
                    && let Some(value) = fields.get(*name)
                {
                    candidate[*name] = value.clone();
                }
            }
            serde_json::from_value::<Settings>(candidate).ok().filter(|s| s.validate(host).is_ok()).map(|s| (mask, s))
        });
        if let Some((mask, recovered)) = found {
            current = recovered;
            for (i, name) in linked.iter().enumerate() {
                if mask & (1 << i) == 0 && fields.contains_key(*name) {
                    reset.push((*name).to_owned());
                }
            }
            break;
        }
    }
    let mut names: Vec<&String> = fields.keys().filter(|name| !linked.contains(&name.as_str())).collect();
    names.sort();
    for name in names {
        let mut candidate = serde_json::to_value(&current).expect("settings serialize");
        let Some(slot) = candidate.get_mut(name.as_str()) else { continue }; // unknown: ignored
        *slot = fields[name].clone();
        match serde_json::from_value::<Settings>(candidate) {
            Ok(s) if s.validate(host).is_ok() => current = s,
            _ => reset.push(name.clone()),
        }
    }
    reset.sort();
    (current, reset)
}
