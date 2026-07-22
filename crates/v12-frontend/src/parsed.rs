//! ParsedProgram: wraps the Oxc allocator + parse result so that AST lifetimes
//! stay valid for the duration of compilation.

use anyhow::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;

use crate::error::FrontendError;

/// Owns the allocator and the source string so the AST `'a` lifetime is valid
/// for as long as this struct lives.
pub struct ParsedProgram {
    /// The original source text.
    pub source: String,
    /// Oxc arena allocator that owns the AST nodes.
    pub allocator: Allocator,
}

impl ParsedProgram {
    /// Parse a JavaScript source string.
    pub fn from_source(source: impl Into<String>) -> Result<Self> {
        let source = source.into();
        let allocator = Allocator::default();

        // We parse here to check for errors; the actual AST reference is
        // borrowed later via `program()`.
        {
            let source_type = SourceType::default().with_module(true);
            let opts = ParseOptions {
                parse_regular_expression: true,
                ..Default::default()
            };
            let ret = Parser::new(&allocator, &source, source_type)
                .with_options(opts)
                .parse();

            if ret.panicked {
                return Err(FrontendError::ParseError(
                    "parser panicked on this input".to_string(),
                )
                .into());
            }

            if !ret.errors.is_empty() {
                let msgs: Vec<String> = ret
                    .errors
                    .iter()
                    .map(|e| format!("{}", e.message))
                    .collect();
                return Err(FrontendError::ParseError(msgs.join("; ")).into());
            }
        }

        Ok(Self { source, allocator })
    }

    /// Re-parse and return the AST program, borrowing from `self`.
    ///
    /// We re-parse here because the `Program<'a>` borrows from both the
    /// allocator and the source string. The allocator must outlive the program,
    /// so we tie the lifetime to `&self`.
    pub fn program<'a>(&'a self) -> Program<'a> {
        let source_type = SourceType::default().with_module(true);
        let opts = ParseOptions {
            parse_regular_expression: true,
            ..Default::default()
        };
        Parser::new(&self.allocator, &self.source, source_type)
            .with_options(opts)
            .parse()
            .program
    }
}
