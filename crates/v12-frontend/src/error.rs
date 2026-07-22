//! Error types for the v12-frontend crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrontendError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Semantic error: {0}")]
    SemanticError(String),
}
