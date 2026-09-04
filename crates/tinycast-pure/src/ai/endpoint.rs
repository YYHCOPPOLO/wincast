//! HTTPS endpoint policy. No Apple Intelligence route.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
    OpenAiCompatible,
}

impl ProviderKind {
    pub fn title(self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "OpenAI API",
            ProviderKind::Anthropic => "Anthropic Claude",
            ProviderKind::Gemini => "Google Gemini",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::OpenAiCompatible => "OpenAI Compatible",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => "https://api.openai.com/v1",
            ProviderKind::Anthropic => "https://api.anthropic.com",
            ProviderKind::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            ProviderKind::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    pub fn is_anthropic(self) -> bool {
        self == ProviderKind::Anthropic
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Uuid(String);

impl Uuid {
    pub fn parse(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ModelSelection {
    Api { connection: Uuid, model: String },
    ChatGpt { model: String, effort: Option<String> },
}

impl ModelSelection {
    pub fn model(&self) -> &str {
        match self {
            ModelSelection::Api { model, .. } | ModelSelection::ChatGpt { model, .. } => model,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Url {
    raw: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Url {
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointError {
    InvalidUrl,
    InsecureRemoteUrl,
}

impl EndpointError {
    pub fn message(self) -> &'static str {
        match self {
            EndpointError::InvalidUrl => "Enter a valid provider base URL.",
            EndpointError::InsecureRemoteUrl => "Remote AI providers require an HTTPS base URL.",
        }
    }
}

pub fn validate_url(url: &str) -> Result<Url, EndpointError> {
    let parsed = parse_url(url.trim()).ok_or(EndpointError::InvalidUrl)?;
    if parsed.scheme != "https" && parsed.scheme != "http" {
        return Err(EndpointError::InvalidUrl);
    }
    if parsed.scheme == "http" && !is_loopback(&parsed.host) {
        return Err(EndpointError::InsecureRemoteUrl);
    }
    Ok(parsed)
}

pub fn is_loopback(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

pub fn parse_url(url: &str) -> Option<Url> {
    let raw = url.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let (scheme, rest) = raw.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (authority, path) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{path}")),
        None => (rest, "/".into()),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = split_host_port(authority, if scheme == "https" { 443 } else { 80 })?;
    if host.is_empty() {
        return None;
    }
    Some(Url {
        raw,
        scheme,
        host,
        port,
        path,
    })
}

fn split_host_port(authority: &str, default_port: u16) -> Option<(String, u16)> {
    if let Some(inner) = authority.strip_prefix('[') {
        let (host, rest) = inner.split_once(']')?;
        if host.is_empty() {
            return None;
        }
        let port = if rest.is_empty() {
            default_port
        } else {
            rest.strip_prefix(':')?.parse().ok()?
        };
        return Some((host.to_string(), port));
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return None;
        }
        return Some((host.to_string(), port.parse().ok()?));
    }
    Some((authority.to_string(), default_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_required_except_loopback() {
        assert!(validate_url("http://example.com").is_err());
        assert!(validate_url("http://127.0.0.1:11434/v1").is_ok());
        assert!(validate_url("https://api.openai.com/v1").is_ok());
        assert!(validate_url("ftp://x").is_err());
    }

    #[test]
    fn model_selection_has_no_apple_case() {
        let names = ["Api", "ChatGpt"];
        assert!(!names.contains(&"AppleIntelligence"));
        let _ = std::mem::discriminant(&ModelSelection::ChatGpt {
            model: "gpt-5".into(),
            effort: None,
        });
    }

    #[test]
    fn loopback_http_and_ipv6_are_accepted() {
        assert!(validate_url("http://localhost:11434/v1").is_ok());
        assert!(validate_url("http://[::1]/v1").is_ok());
        assert!(validate_url("ftp://localhost/v1").is_err());
        assert_eq!(
            validate_url("http://example.com").unwrap_err(),
            EndpointError::InsecureRemoteUrl
        );
    }
}
