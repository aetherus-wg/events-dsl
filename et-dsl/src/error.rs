//! Error types for the filter DSL
//!
//! Provides error types for parsing and evaluation failures.

use chumsky::span::SimpleSpan;

#[derive(Debug)]
/// An error that can occur during parsing or evaluation.
///
/// Variants:
/// - `Spanned` - Error with source location information
/// - `Unspanned` - Error without location (e.g., semantic errors)
pub enum Error {
    /// Error with source span
    Spanned { span: SimpleSpan, msg: String },
    /// Error without location
    Unspanned(String),
}

impl Error {
    /// Add a span to an unspanned error.
    ///
    /// If the error is already spanned, returns self unchanged.
    /// Otherwise wraps the message with the given span.
    pub fn with_span(self, span: SimpleSpan) -> Self {
        match self {
            Self::Spanned { .. } => self,
            Self::Unspanned(msg) => Self::Spanned { span, msg },
        }
    }
    /// Get the span of this error, if available.
    pub fn span(&self) -> Option<SimpleSpan> {
        match self {
            Self::Spanned { span, .. } => Some(*span),
            Self::Unspanned(_) => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Spanned { span: _, msg } => write!(f, "Error: {}", msg),
            Error::Unspanned(msg) => write!(f, "Error: {}", msg),
        }
    }
}
