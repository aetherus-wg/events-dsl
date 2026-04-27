pub mod ast;
pub mod error;
pub mod evaluate;
pub mod model;
pub mod parse;
pub mod token;

use crate::{
    ast::{Declaration, Expr},
    parse::expr_parser,
    token::lexer,
};
use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::{Parser, error::Rich, input::Input, span::SimpleSpan};
use itertools::Itertools;
use log::debug;
use std::{collections::HashSet, path::Path};

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

// -----------------------------------------------
// Model traits and helpers
// -----------------------------------------------
pub trait Check<T> {
    fn check(&self, value: T) -> bool;
}

pub fn extract_ledger_path(
    declarations: &Vec<Declaration>,
    script_src: &str,
    script_filepath: &Path,
) -> Option<std::path::PathBuf> {
    let script_dirname = &script_filepath
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut ledger_path = None;
    for decl in declarations.iter() {
        match decl.body {
            (Expr::LedgerPath(path), span) => {
                if let Some((_, first_span)) = ledger_path {
                    failure(
                        "Multiple ledger/photons paths specified".to_string(),
                        ("another declaration here".to_string(), span),
                        [("first declaration here".to_string(), first_span)],
                        script_src,
                    );
                } else {
                    ledger_path = Some((script_dirname.join(path), span));
                }
            }
            _ => (),
        }
    }

    match ledger_path {
        Some((path, _)) => Some(path),
        None => None,
    }
}

pub fn extract_signals_path(
    declarations: &Vec<Declaration>,
    script_src: &str,
    script_filepath: &Path,
) -> Option<std::path::PathBuf> {
    let script_dirname = &script_filepath
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut signals_path = None;
    for decl in declarations.iter() {
        match decl.body {
            (Expr::SignalsPath(path), span) => {
                if let Some((_, first_span)) = signals_path {
                    failure(
                        "Multiple signals paths specified".to_string(),
                        ("another declaration here".to_string(), span),
                        [("first declaration here".to_string(), first_span)],
                        script_src,
                    );
                } else {
                    signals_path = Some((script_dirname.join(path), span));
                }
            }
            _ => (),
        }
    }

    match signals_path {
        Some((path, _)) => Some(path),
        None => None,
    }
}

// -----------------------------------------------
// Parsing entry point
// -----------------------------------------------
pub fn parse_script<'src>(
    script_src: &'src str,
    field_dict: &HashSet<String>,
) -> Vec<Declaration<'src>> {
    let tokens = lexer(&field_dict)
        .parse(script_src)
        .into_result()
        .unwrap_or_else(|errs| {
            errs.iter().for_each(|err| parse_failure(&err, script_src));
            std::process::exit(1)
        });

    debug!("Tokens: {}", tokens.iter().map(|t| t.0.to_string()).join(" "));

    let declarations = expr_parser()
        .parse(
            tokens
                .as_slice()
                .map((script_src.len()..script_src.len()).into(), |(t, s)| (t, s)),
        )
        .into_result()
        .unwrap_or_else(|errs| {
            errs.iter().for_each(|err| parse_failure(&err, script_src));
            std::process::exit(1)
        });

    declarations
}

// -----------------------------------------------
// Parsing helpers
// -----------------------------------------------

pub fn failure(
    msg: String,
    label: (String, SimpleSpan),
    extra_labels: impl IntoIterator<Item = (String, SimpleSpan)>,
    src: &str,
) -> ! {
    let fname = "example";
    Report::build(ReportKind::Error, (fname, label.1.into_range()))
        .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
        .with_message(&msg)
        .with_label(
            Label::new((fname, label.1.into_range()))
                .with_message(label.0)
                .with_color(Color::Red),
        )
        .with_labels(extra_labels.into_iter().map(|label2| {
            Label::new((fname, label2.1.into_range()))
                .with_message(label2.0)
                .with_color(Color::Yellow)
        }))
        .finish()
        .print(sources([(fname, src)]))
        .unwrap();
    std::process::exit(1)
}

pub fn parse_failure(err: &Rich<impl std::fmt::Display>, src: &str) -> ! {
    failure(
        err.reason().to_string(),
        (
            err.found()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "end of input".to_string()),
            *err.span(),
        ),
        err.contexts()
            .map(|(l, s)| (format!("while parsing this {l}"), *s)),
        src,
    )
}
