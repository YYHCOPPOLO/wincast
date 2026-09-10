use tinycast_pure::ai::{
    url_is_loopback, validate_url, AiConnection, ModelSelection, ProviderKind,
};

use super::keys::ApiKeyStore;
use super::provider::HttpAiProvider;
use windows::Win32::Foundation::HWND;

pub struct ProviderFactory {
    keys: ApiKeyStore,
    host: HWND,
}

impl ProviderFactory {
    pub fn new(host: HWND) -> Self {
        Self {
            keys: ApiKeyStore::open(),
            host,
        }
    }

    pub fn set_host(&mut self, host: HWND) {
        self.host = host;
    }

    pub fn keys(&self) -> &ApiKeyStore {
        &self.keys
    }

    pub fn keys_mut(&mut self) -> &mut ApiKeyStore {
        &mut self.keys
    }

    /// Reads the key at the last moment. ChatGPT is wired in the settings task.
    pub fn http_provider(
        &self,
        selection: &ModelSelection,
        connections: &[AiConnection],
    ) -> Result<HttpAiProvider, String> {
        match selection {
            ModelSelection::ChatGpt { .. } => Err(
                "ChatGPT uses Codex on PATH. Connect it in Settings, or choose an API connection."
                    .into(),
            ),
            ModelSelection::Api { connection, model } => {
                let conn = connections
                    .iter()
                    .find(|c| c.id.as_str() == connection.as_str())
                    .ok_or_else(|| "Choose an API connection in Settings.".to_string())?;
                self.http_for_connection(conn, model.clone())
            }
        }
    }

    pub fn http_for_connection(
        &self,
        connection: &AiConnection,
        model: String,
    ) -> Result<HttpAiProvider, String> {
        let _ = validate_url(&connection.base_url).map_err(|e| e.message().to_string())?;
        let key = self.keys.get(&connection.id).unwrap_or_default();
        if key.is_empty() && !url_is_loopback(&connection.base_url) {
            return Err("Add an API key in Settings.".into());
        }
        Ok(HttpAiProvider::new(
            connection.provider,
            &connection.base_url,
            model,
            key,
            self.host,
        ))
    }

    pub fn forget_if_retargeted(&self, previous: &AiConnection, next: &AiConnection) {
        if tinycast_pure::ai::should_forget_key(previous, next) {
            self.keys.remove(&previous.id);
        }
    }
}

pub fn default_provider_kind() -> ProviderKind {
    ProviderKind::OpenAi
}
