//! Fetch public Gemini installed-client metadata once, directly from the pinned official source.
use crate::domain::accounts::{GoogleOAuthClient, parse_google_oauth_source};

static CLIENT: tokio::sync::OnceCell<GoogleOAuthClient> = tokio::sync::OnceCell::const_new();
const SOURCE: &str =
    "https://raw.githubusercontent.com/google-gemini/gemini-cli/v0.64.0/packages/core/src/code_assist/oauth2.ts";

pub async fn load() -> Result<&'static GoogleOAuthClient, String> {
    CLIENT
        .get_or_try_init(|| async {
            let mut response = super::super::client()?
                .get(SOURCE)
                .send()
                .await
                .map_err(|_| "Google public OAuth metadata unavailable; check internet access and retry")?;
            if !response.status().is_success() {
                return Err("Google public OAuth metadata unavailable; retry later".into());
            }
            if response.content_length().is_some_and(|n| n > 131072) {
                return Err("Google public OAuth metadata is too large".into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) =
                response.chunk().await.map_err(|_| "Google public OAuth metadata read failed; retry")?
            {
                if bytes.len().saturating_add(chunk.len()) > 131072 {
                    return Err("Google public OAuth metadata is too large".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let source = std::str::from_utf8(&bytes).map_err(|_| "Google public OAuth metadata is malformed")?;
            let config = parse_google_oauth_source(source)?;
            config.validate()?;
            Ok(config)
        })
        .await
}
