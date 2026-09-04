#[derive(Debug, thiserror::Error)]
pub enum DiffenatorError {
    #[error("FontDrasil error: {0}")]
    FontDrasilError(#[from] fontdrasil::error::Error),
    #[error("Font read error: {0}")]
    FontReadError(#[from] read_fonts::ReadError),
}
