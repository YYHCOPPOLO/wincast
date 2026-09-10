use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use tinycast_pure::ai::{
    anthropic_body, completion_endpoint, openai_body, url_is_loopback, AiEvent, AiRequest,
    ProviderKind, StreamDecoder, StreamShape,
};

use crate::platform::messages::WM_AI;
use crate::platform::winhttp;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub trait AiProvider {
    fn stream(&mut self, req: AiRequest, sink: Sender<AiEvent>);
    fn cancel(&mut self);
}

pub struct HttpAiProvider {
    provider: ProviderKind,
    endpoint: String,
    model: String,
    api_key: String,
    host: HWND,
    cancel: Arc<AtomicBool>,
}

impl std::fmt::Debug for HttpAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpAiProvider")
            .field("provider", &self.provider)
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl HttpAiProvider {
    pub fn new(
        provider: ProviderKind,
        base_url: &str,
        model: String,
        api_key: String,
        host: HWND,
    ) -> Self {
        Self {
            provider,
            endpoint: completion_endpoint(provider, base_url),
            model,
            api_key,
            host,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AiProvider for HttpAiProvider {
    fn stream(&mut self, req: AiRequest, sink: Sender<AiEvent>) {
        self.cancel.store(false, Ordering::SeqCst);
        let cancel = Arc::clone(&self.cancel);
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        let key = self.api_key.clone();
        let provider = self.provider;
        let host_bits = self.host.0 as isize;
        let _ = std::thread::Builder::new()
            .name("tinycast-ai".into())
            .spawn(move || {
                run_stream(provider, endpoint, model, key, req, sink, cancel, host_bits)
            });
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

fn run_stream(
    provider: ProviderKind,
    endpoint: String,
    model: String,
    key: String,
    req: AiRequest,
    sink: Sender<AiEvent>,
    cancel: Arc<AtomicBool>,
    host_bits: isize,
) {
    let shape = if provider.is_anthropic() {
        StreamShape::Anthropic
    } else {
        StreamShape::OpenAiCompatible
    };
    let body = if provider.is_anthropic() {
        anthropic_body(
            &model,
            req.instructions.as_deref(),
            &req.messages,
            req.max_output_tokens,
        )
    } else {
        openai_body(
            &model,
            req.instructions.as_deref(),
            &req.messages,
            req.web_search,
            provider == ProviderKind::OpenRouter,
            req.max_output_tokens,
        )
    };
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    let mut headers: Vec<(String, String)> = vec![
        ("Content-Type".into(), "application/json".into()),
        ("Accept".into(), "text/event-stream".into()),
    ];
    if provider.is_anthropic() {
        if !key.is_empty() {
            headers.push(("x-api-key".into(), key));
        }
        headers.push(("anthropic-version".into(), "2023-06-01".into()));
    } else {
        if !key.is_empty() {
            headers.push(("Authorization".into(), format!("Bearer {key}")));
        }
        if provider == ProviderKind::Gemini {
            headers.push(("x-goog-api-client".into(), "tinycast-oai/0.10.2".into()));
        } else if provider == ProviderKind::OpenRouter {
            headers.push(("X-OpenRouter-Title".into(), "Tinycast".into()));
        }
    }
    let header_refs: Vec<(&str, &str)> = headers
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut decoder = StreamDecoder::new(shape);
    let result = winhttp::post_stream(&endpoint, &header_refs, &bytes, |chunk| {
        if cancel.load(Ordering::SeqCst) {
            return Ok(false);
        }
        match decoder.feed(chunk) {
            Ok(events) => {
                for event in events {
                    if emit(&sink, event, host_bits) {
                        return Ok(false);
                    }
                }
                Ok(!decoder.is_terminal())
            }
            Err(err) => {
                let _ = emit(&sink, AiEvent::Error(err), host_bits);
                Ok(false)
            }
        }
    });
    if cancel.load(Ordering::SeqCst) {
        return;
    }
    match result {
        Ok(_) => {
            if !decoder.is_terminal() {
                match decoder.finish() {
                    Ok(events) => {
                        for event in events {
                            let _ = emit(&sink, event, host_bits);
                        }
                    }
                    Err(err) => {
                        let _ = emit(&sink, AiEvent::Error(err), host_bits);
                    }
                }
            }
            if !decoder.is_terminal() {
                let _ = emit(
                    &sink,
                    AiEvent::Error("The connection closed before the response completed.".into()),
                    host_bits,
                );
            }
        }
        Err(err) => {
            let _ = emit(&sink, AiEvent::Error(err), host_bits);
        }
    }
}

fn emit(sink: &Sender<AiEvent>, event: AiEvent, host_bits: isize) -> bool {
    let stop = matches!(event, AiEvent::Done | AiEvent::Error(_));
    if sink.send(event).is_err() {
        return true;
    }
    if host_bits != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(host_bits as *mut core::ffi::c_void),
                WM_AI,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
    stop
}

pub fn loopback_allows_empty_key(base_url: &str) -> bool {
    url_is_loopback(base_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::ai::AiRequest;

    #[test]
    fn debug_omits_api_key() {
        let provider = HttpAiProvider::new(
            ProviderKind::OpenAi,
            "https://api.openai.com/v1",
            "gpt-4.1-mini".into(),
            "sk-never-appear".into(),
            HWND::default(),
        );
        let text = format!("{provider:?}");
        assert!(!text.contains("sk-never-appear"));
        assert!(text.contains("gpt-4.1-mini"));
        let _ = AiRequest::new(vec![]);
    }
}
