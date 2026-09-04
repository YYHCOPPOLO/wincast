pub mod connection;
pub mod endpoint;
pub mod instructions;
pub mod open_policy;
pub mod request;
pub mod session;
pub mod stream;

pub use connection::{completion_endpoint, should_forget_key, AiConnection};
pub use endpoint::{
    same_destination, url_is_loopback, validate_url, EndpointError, ModelSelection, ProviderKind,
    Url, Uuid,
};
pub use instructions::compose_instructions;
pub use open_policy::{should_resume, OpenPolicy};
pub use request::{AiEvent, AiMessage, AiRequest, Role};
pub use session::{
    bounded_context, ChatConversation, ChatMessage, ChatRole, ChatSession, ChatState,
    DEFAULT_TEXT_BUDGET,
};
pub use stream::{anthropic_body, openai_body, StreamDecoder, StreamShape};
