use crate::ai::endpoint::{same_destination, ProviderKind, Uuid};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConnection {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    pub provider: ProviderKind,
    #[serde(rename = "baseURL")]
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub vision_models: Vec<String>,
}

impl AiConnection {
    pub fn new(provider: ProviderKind) -> Self {
        Self {
            id: Uuid::generate(),
            name: String::new(),
            provider,
            base_url: provider.default_base_url().to_string(),
            models: Vec::new(),
            vision_models: Vec::new(),
        }
    }

    pub fn title(&self) -> String {
        let trimmed = self.name.trim();
        if trimmed.is_empty() {
            self.provider.title().to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// Changing provider or host forgets the saved key rather than send it elsewhere.
pub fn should_forget_key(previous: &AiConnection, next: &AiConnection) -> bool {
    previous.provider != next.provider || !same_destination(&previous.base_url, &next.base_url)
}

pub fn completion_endpoint(provider: ProviderKind, base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if provider.is_anthropic() {
        if base.ends_with("/messages") {
            base.to_string()
        } else {
            format!("{base}/v1/messages")
        }
    } else if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_resolve_streaming_endpoints() {
        let expected = [
            (
                ProviderKind::OpenAi,
                "https://api.openai.com/v1/chat/completions",
            ),
            (
                ProviderKind::Anthropic,
                "https://api.anthropic.com/v1/messages",
            ),
            (
                ProviderKind::Gemini,
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            ),
            (
                ProviderKind::OpenRouter,
                "https://openrouter.ai/api/v1/chat/completions",
            ),
            (
                ProviderKind::OpenAiCompatible,
                "https://api.openai.com/v1/chat/completions",
            ),
        ];
        for (provider, endpoint) in expected {
            assert_eq!(
                completion_endpoint(provider, provider.default_base_url()),
                endpoint,
                "{}",
                provider.title()
            );
        }
        assert_eq!(
            completion_endpoint(
                ProviderKind::OpenAiCompatible,
                "https://example.com/chat/completions"
            ),
            "https://example.com/chat/completions"
        );
    }

    #[test]
    fn retarget_forgets_key_when_provider_or_host_changes() {
        let saved = AiConnection::new(ProviderKind::OpenAi);
        let mut renamed = saved.clone();
        renamed.name = "Work".into();
        renamed.models = vec!["gpt-4.1-mini".into()];
        assert!(!should_forget_key(&saved, &renamed));
        let mut switched = saved.clone();
        switched.provider = ProviderKind::Anthropic;
        assert!(should_forget_key(&saved, &switched));
        let mut url = saved.clone();
        url.base_url = "https://gateway.example.com/v1".into();
        assert!(should_forget_key(&saved, &url));
    }
}
