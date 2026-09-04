pub mod dfont;
pub mod error;
pub mod gposdiff;
pub mod gsubdiff;
pub mod staticdiff;
pub mod structs;
pub mod summary;
pub mod wordselect;
// Shared HTML rendering/templating code
#[cfg(feature = "html")]
pub mod html;
pub mod render;
pub use static_lang_word_lists::WordList;
pub use summary::{summarize, SignatureSummary};
