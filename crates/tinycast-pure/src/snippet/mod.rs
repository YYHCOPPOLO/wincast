pub mod codec;
pub mod model;

pub use codec::{parse_markdown, serialize, CodecError};
pub use model::{default_name, slug, SnippetSourceRevision, StoredSnippet};
