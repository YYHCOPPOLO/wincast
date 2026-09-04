use crate::ai::request::AiEvent;

pub struct SseParser {
    buffer: Vec<u8>,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseParser {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn feed(&mut self, data: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(data);
        let mut payloads = Vec::new();
        while let Some((start, end)) = next_boundary(&self.buffer) {
            let frame = self.buffer[..start].to_vec();
            self.buffer.drain(..end);
            if let Some(payload) = payload_in(&frame) {
                payloads.push(payload);
            }
        }
        payloads
    }

    pub fn finish(&mut self) -> Vec<String> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let frame = std::mem::take(&mut self.buffer);
        payload_in(&frame).into_iter().collect()
    }
}

fn next_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = find_subslice(buffer, b"\n\n").map(|i| (i, i + 2));
    let crlf = find_subslice(buffer, b"\r\n\r\n").map(|i| (i, i + 4));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(if a.0 < b.0 { a } else { b }),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn payload_in(frame: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(frame).replace("\r\n", "\n");
    let mut data_lines = Vec::new();
    for line in text.split('\n') {
        if line.starts_with(':') {
            continue;
        }
        if line == "data" {
            data_lines.push(String::new());
        } else if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.strip_prefix(' ').unwrap_or(value).to_string());
        }
    }
    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamShape {
    OpenAiCompatible,
    Anthropic,
}

pub struct StreamDecoder {
    shape: StreamShape,
    parser: SseParser,
    terminal: bool,
}

impl StreamDecoder {
    pub fn new(shape: StreamShape) -> Self {
        Self {
            shape,
            parser: SseParser::new(),
            terminal: false,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<AiEvent>, String> {
        let payloads = self.parser.feed(data);
        self.decode(payloads)
    }

    pub fn finish(&mut self) -> Result<Vec<AiEvent>, String> {
        let payloads = self.parser.finish();
        self.decode(payloads)
    }

    fn decode(&mut self, payloads: Vec<String>) -> Result<Vec<AiEvent>, String> {
        let mut events = Vec::new();
        for payload in payloads {
            if self.terminal {
                break;
            }
            if payload == "[DONE]" {
                self.terminal = true;
                events.push(AiEvent::Done);
                continue;
            }
            match self.shape {
                StreamShape::OpenAiCompatible => {
                    events.extend(self.decode_openai(&payload)?);
                }
                StreamShape::Anthropic => {
                    events.extend(self.decode_anthropic(&payload)?);
                }
            }
        }
        Ok(events)
    }

    fn decode_openai(&mut self, payload: &str) -> Result<Vec<AiEvent>, String> {
        let chunk: serde_json::Value = serde_json::from_str(payload).map_err(|_| {
            self.terminal = true;
            "The provider returned malformed streaming data.".to_string()
        })?;
        if let Some(message) = chunk
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            self.terminal = true;
            return Err(message.to_string());
        }
        let mut events = Vec::new();
        if let Some(delta) = chunk
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("delta"))
        {
            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                if !content.is_empty() {
                    events.push(AiEvent::Text(content.to_string()));
                }
            } else if has_reasoning(delta) {
                events.push(AiEvent::Thinking(true));
            }
        }
        if chunk.get("usage").is_some() {
            events.push(AiEvent::Usage);
        }
        Ok(events)
    }

    fn decode_anthropic(&mut self, payload: &str) -> Result<Vec<AiEvent>, String> {
        let event: serde_json::Value = serde_json::from_str(payload).map_err(|_| {
            self.terminal = true;
            "The provider returned malformed streaming data.".to_string()
        })?;
        let kind = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match kind {
            "content_block_delta" => {
                let delta = event.get("delta");
                let dtype = delta
                    .and_then(|d| d.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if dtype == "text_delta" {
                    if let Some(text) = delta.and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
                        if !text.is_empty() {
                            return Ok(vec![AiEvent::Text(text.to_string())]);
                        }
                    }
                    Ok(Vec::new())
                } else if dtype == "thinking_delta" {
                    Ok(vec![AiEvent::Thinking(true)])
                } else {
                    Ok(Vec::new())
                }
            }
            "message_start" | "message_delta" => Ok(vec![AiEvent::Usage]),
            "message_stop" => {
                self.terminal = true;
                Ok(vec![AiEvent::Done])
            }
            "error" => {
                self.terminal = true;
                let typ = event
                    .get("error")
                    .and_then(|e| e.get("type"))
                    .and_then(|t| t.as_str());
                Err(anthropic_error(typ))
            }
            _ => Ok(Vec::new()),
        }
    }
}

fn has_reasoning(delta: &serde_json::Value) -> bool {
    if delta
        .get("reasoning")
        .and_then(|r| r.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    delta
        .get("reasoning_details")
        .and_then(|d| d.as_array())
        .map(|items| {
            items.iter().any(|item| {
                item.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn anthropic_error(kind: Option<&str>) -> String {
    match kind {
        Some("authentication_error") => "API key rejected — check it in Settings.".into(),
        Some("rate_limit_error") => "Rate limit reached — try again later.".into(),
        _ => "The provider stopped the response with an error.".into(),
    }
}

pub fn openai_body(
    model: &str,
    instructions: Option<&str>,
    messages: &[crate::ai::request::AiMessage],
    web_search: bool,
    openrouter: bool,
) -> serde_json::Value {
    let mut payload = Vec::new();
    if let Some(text) = instructions.map(str::trim).filter(|s| !s.is_empty()) {
        payload.push(serde_json::json!({"role": "system", "content": text}));
    }
    for message in messages {
        let text = message.text.trim();
        if text.is_empty() {
            continue;
        }
        payload.push(serde_json::json!({
            "role": message.role.as_str(),
            "content": text,
        }));
    }
    let mut value = serde_json::json!({
        "model": model,
        "messages": payload,
        "stream": true,
    });
    if web_search && openrouter {
        value["plugins"] = serde_json::json!([{"id": "web"}]);
    }
    value
}

pub fn anthropic_body(
    model: &str,
    instructions: Option<&str>,
    messages: &[crate::ai::request::AiMessage],
    max_output_tokens: u32,
) -> serde_json::Value {
    use crate::ai::request::Role;
    let mut system_parts = Vec::new();
    if let Some(text) = instructions.map(str::trim).filter(|s| !s.is_empty()) {
        system_parts.push(text.to_string());
    }
    let mut payload = Vec::new();
    for message in messages {
        if message.role == Role::System {
            let text = message.text.trim();
            if !text.is_empty() {
                system_parts.push(text.to_string());
            }
            continue;
        }
        let text = message.text.trim();
        if text.is_empty() {
            continue;
        }
        payload.push(serde_json::json!({
            "role": message.role.as_str(),
            "content": text,
        }));
    }
    let mut value = serde_json::json!({
        "model": model,
        "messages": payload,
        "max_tokens": max_output_tokens,
        "stream": true,
    });
    if !system_parts.is_empty() {
        value["system"] = serde_json::Value::String(system_parts.join("\n\n"));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_frames_survive_splits() {
        let mut parser = SseParser::new();
        assert!(parser.feed(b"data: hel").is_empty());
        assert_eq!(
            parser.feed(b"lo\n\ndata: world\r\n\r\n"),
            vec!["hello".to_string(), "world".to_string()]
        );
        assert!(parser.feed(b": keepalive\n\ndata: final").is_empty());
        assert_eq!(parser.finish(), vec!["final".to_string()]);
    }

    #[test]
    fn openai_and_anthropic_streams_decode() {
        let mut open_ai = StreamDecoder::new(StreamShape::OpenAiCompatible);
        let data = br#"data: {"choices":[{"delta":{"reasoning":"working"}}]}

data: {"choices":[{"delta":{"content":"Hello"}}]}

data: {"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2}}

data: [DONE]

"#;
        let mut events = open_ai.feed(data).unwrap();
        events.extend(open_ai.finish().unwrap());
        assert!(events.contains(&AiEvent::Thinking(true)));
        assert!(events.contains(&AiEvent::Text("Hello".into())));
        assert!(events.contains(&AiEvent::Usage));
        assert_eq!(events.last(), Some(&AiEvent::Done));

        let mut anthropic = StreamDecoder::new(StreamShape::Anthropic);
        let data = br#"data: {"type":"message_start","message":{"usage":{"input_tokens":4}}}

data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}

data: {"type":"message_delta","usage":{"output_tokens":1}}

data: {"type":"message_stop"}

"#;
        let mut events = anthropic.feed(data).unwrap();
        events.extend(anthropic.finish().unwrap());
        assert!(events.contains(&AiEvent::Text("Hi".into())));
        assert!(events.contains(&AiEvent::Usage));
        assert_eq!(events.last(), Some(&AiEvent::Done));
    }
}
