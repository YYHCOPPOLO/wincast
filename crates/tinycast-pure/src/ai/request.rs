#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "system" => Some(Role::System),
            "user" => Some(Role::User),
            "assistant" => Some(Role::Assistant),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiMessage {
    pub role: Role,
    pub text: String,
}

impl AiMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            text: text.into(),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            text: text.into(),
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiRequest {
    pub instructions: Option<String>,
    pub messages: Vec<AiMessage>,
    pub max_output_tokens: u32,
    pub web_search: bool,
}

impl AiRequest {
    pub fn new(messages: Vec<AiMessage>) -> Self {
        Self {
            instructions: None,
            messages,
            max_output_tokens: 4096,
            web_search: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiEvent {
    Text(String),
    Thinking(bool),
    Usage,
    Done,
    Error(String),
}
