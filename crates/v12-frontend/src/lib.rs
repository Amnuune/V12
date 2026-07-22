//! v12-frontend: Wraps Oxc to produce a parsed, semantically-analyzed JS program.

pub mod error;
pub mod parsed;

pub use parsed::ParsedProgram;
