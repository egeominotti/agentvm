//! Provider credentials use separate Keychain services and standard host environment fallbacks.
use super::{Keychain, KeychainError};
use crate::domain::providers::Provider;
use crate::secret::Secret;

impl Keychain {
    pub fn read_provider_key(&self, provider: Provider) -> Option<Secret> {
        self.read(&service(provider)).or_else(|| {
            env_names(provider).iter().find_map(|name| {
                std::env::var(name).ok().filter(|s| !s.trim().is_empty()).map(|s| Secret::new(s.trim().to_owned()))
            })
        })
    }
    /// The stored value only, without environment fallback, for compensating failed updates.
    pub fn read_stored_provider_key(&self, provider: Provider) -> Result<Option<Secret>, KeychainError> {
        self.read_checked(&service(provider))
    }
    pub fn write_provider_key(&self, provider: Provider, secret: &Secret) -> Result<(), KeychainError> {
        let value = secret.expose();
        if value.is_empty()
            || value.len() > 4096
            || !value.bytes().all(|c| c.is_ascii_graphic() && c != b'"' && c != b'\\')
        {
            return Err(KeychainError::InvalidProviderKey);
        }
        self.write(&service(provider), &format!("\"{value}\""))
    }
    pub fn delete_provider_key(&self, provider: Provider) -> Result<(), KeychainError> {
        self.delete(&service(provider))
    }
    pub fn has_provider_key(&self, provider: Provider) -> bool {
        self.read_provider_key(provider).is_some()
    }
}
fn service(provider: Provider) -> String {
    format!("agentvm-provider-{}", provider.id())
}
fn env_names(provider: Provider) -> &'static [&'static str] {
    match provider {
        Provider::Openai => &["OPENAI_API_KEY"],
        Provider::Anthropic => &["ANTHROPIC_API_KEY"],
        Provider::Google => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        Provider::Openrouter => &["OPENROUTER_API_KEY"],
        Provider::Alibabacodingplan => &["BAILIAN_CODING_PLAN_API_KEY"],
        Provider::Opencodego => &["OPENCODE_GO_API_KEY"],
        Provider::Opencodezen => &["OPENCODE_ZEN_API_KEY"],
        Provider::Local => &[],
        Provider::Custom => &["AGENTVM_CUSTOM_API_KEY"],
    }
}
