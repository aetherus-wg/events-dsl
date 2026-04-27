use chumsky::span::SimpleSpan;

pub enum Error {
    Spanned { span: SimpleSpan, msg: String },
    Unspanned(String),
}

impl Error {
    pub fn with_span(self, span: SimpleSpan) -> Self {
        match self {
            Self::Spanned { .. } => self,
            Self::Unspanned(msg) => Self::Spanned { span, msg },
        }
    }
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
            Error::Spanned { span, msg } => write!(f, "Error: {}", msg),
            Error::Unspanned(msg) => write!(f, "Error: {}", msg),
        }
    }
}
