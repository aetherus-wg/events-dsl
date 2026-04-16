use std::{env, fmt, fs};
use ariadne::{sources, Color, Label, Report, ReportKind};
use filter_dsl::{parse::expr_parser, token::lexer};
use itertools::Itertools;

use chumsky::prelude::*;

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
    let encoding_filename = env::args().nth(1).expect("Expected file argument for encoding scheme");
    let encoding_src = &fs::read_to_string(&encoding_filename).expect("Failed to read encoding scheme file");

    let script_filename = env::args().nth(2).expect("Expected file argument for DSL script");
    let script_src = &fs::read_to_string(&script_filename).expect("Failed to read script file");

    let trie = encoding_spec::build_decoder(encoding_src).expect("Failed to build decoder from encoding scheme");

    let dict = trie.get_fields();
    println!("FieldId dictionary: {:?}", dict);

    let tokens = lexer(&dict)
        .parse(script_src)
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], script_src));

    println!("Tokens: {}", tokens.iter().map(|t| t.0.to_string()).join(" "));
    //println!("Tokens: {:?}", tokens);

    let declarations = expr_parser()
        .parse(
            tokens
                .as_slice()
                .map((script_src.len()..script_src.len()).into(), |(t, s)| (t, s)),
        )
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], script_src));

    //println!("Declarations: {:?}", declarations);
}
