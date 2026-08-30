pub mod codec;
pub mod keyword;
pub mod model;

pub use codec::{parse_markdown, serialize, CodecError};
pub use keyword::{classify_input, match_suffix, KeywordBuffer, KeywordInput};
pub use model::{default_name, slug, SnippetSourceRevision, StoredSnippet};
