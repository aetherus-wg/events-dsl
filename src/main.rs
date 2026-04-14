use std::{collections::{HashMap, HashSet}, env, fmt::{self, Debug}, fs};
use ariadne::{sources, Color, Label, Report, ReportKind};
use filter_dsl::{expr::expr_parser, tokenizer::lexer};
use itertools::Itertools;

use chumsky::{input::ValueInput, prelude::*};
use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};

/// -------------------------------------------------
/// Semantic Model
/// -------------------------------------------------

//pub enum Expr<'src> {
//    Field(Field<'src>),
//    // (pipeline, event, src_id)
//    //  - event can be Concat(..) = SuperEvent | SubEvent | SubSubEvent
//    Pattern((Field<'src>, Box<Expr<'src>>, Field<'src>)),
//    Any(Vec<Expr<'src>>),
//    Perm(Vec<Expr<'src>>),
//    Seq(Vec<Expr<'src>>),
//    Qualifier(Vec<Expr<'src>>),
//    Concat(Box<Expr<'src>>, Box<Expr<'src>>),
//}

/// -------------------------------------------------
/// Control Domain Model from Semantics Model
/// -------------------------------------------------

//pub struct BitsMatch {
//    mask: u32,
//    value: u32,
//}
//
//pub enum PatternMatch {
//    Positive(BitsMatch),
//    Negative(BitsMatch),
//    Composite{pos: BitsMatch, neg: BitsMatch},
//}

// -----------------------------------------------
// Parsing helpers
// -----------------------------------------------

fn failure(
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

fn parse_failure(err: &Rich<impl fmt::Display>, src: &str) -> ! {
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

fn main() {
    let filename = env::args().nth(1).expect("Expected file argument");
    let src = &fs::read_to_string(&filename).expect("Failed to read file");

    let dict = HashSet::from([
        "MCRT".to_string(),
        "Emission".to_string(),
        "Detection".to_string(),
        "Material".to_string(),
        "Interface".to_string(),
        "Reflector".to_string(),
        "Elastic".to_string(),
        "Inelastic".to_string(),
        "HenyeyGreenstein".to_string(),
        "Rayleigh".to_string(),
        "Mie".to_string(),
        "Raman".to_string(),
        "Fluorescence".to_string(),
        "Forward".to_string(),
        "Backward".to_string(),
        "Side".to_string(),
        "Unknown".to_string(),
    ]);

    let tokens = lexer(dict)
        .parse(src)
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], src));

    println!("Tokens: {}", tokens.iter().map(|t| t.0.to_string()).join(" "));
    //println!("Tokens: {:?}", tokens);

    let declarations = expr_parser()
        .parse(
            tokens
                .as_slice()
                .map((src.len()..src.len()).into(), |(t, s)| (t, s)),
        )
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], src));

    println!("Declarations: {:?}", declarations);

}
