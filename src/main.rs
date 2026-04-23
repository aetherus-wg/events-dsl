use std::{collections::HashMap, env, fmt, fs, path::Path};
use aetherus_events::{ledger::Uid, reader::{read_csv, read_ledger}};
use ariadne::{sources, Color, Label, Report, ReportKind};
use env_logger::Env;
use filter_dsl::{ast::{Declaration, Expr}, model::{find_forward_uid_rule, resolve_ast}, parse::expr_parser, token::lexer};
use itertools::Itertools;

use chumsky::prelude::*;
use log::{debug, info};

pub fn extract_ledger_path(declarations: &Vec<Declaration>, script_src: &str, script_filepath: &Path) -> Option<std::path::PathBuf> {
    let script_dirname = &script_filepath.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut ledger_path = None;
    for decl in declarations.iter() {
        match decl.body {
            (Expr::LedgerPath(path), span) => {
                if let Some((_, first_span)) = ledger_path {
                        failure("Multiple ledger/photons paths specified".to_string(),
                        ("another declaration here".to_string(), span),
                        [("first declaration here".to_string(), first_span)],
                        script_src,
                    );
                } else {
                    ledger_path = Some((script_dirname.join(path), span));
                }
            },
            _                           => (),
        }
    }

    match ledger_path {
        Some((path, _)) => Some(path),
        None => None,
    }
}

pub fn extract_signals_path(declarations: &Vec<Declaration>, script_src: &str, script_filepath: &Path) -> Option<std::path::PathBuf> {
    let script_dirname = &script_filepath.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut signals_path = None;
    for decl in declarations.iter() {
        match decl.body {
            (Expr::SignalsPath(path), span) => {
                if let Some((_, first_span)) = signals_path {
                        failure("Multiple signals paths specified".to_string(),
                        ("another declaration here".to_string(), span),
                        [("first declaration here".to_string(), first_span)],
                        script_src,
                    );
                } else {
                    signals_path = Some((script_dirname.join(path), span));
                }
            },
            _                           => (),
        }
    }

    match signals_path {
        Some((path, _)) => Some(path),
        None => None,
    }
}


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
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let encoding_filename = env::args().nth(1).expect("Expected file argument for encoding scheme");
    let encoding_src = &fs::read_to_string(&encoding_filename).expect("Failed to read encoding scheme file");

    let script_arg = env::args().nth(2).expect("Expected file argument for DSL script");
    let script_filepath = Path::new(&script_arg);
    let script_src = &fs::read_to_string(&script_filepath).expect("Failed to read script file");

    let trie = encoding_spec::build_decoder(encoding_src).expect("Failed to build decoder from encoding scheme");

    let dict = trie.get_fields();
    info!("FieldId dictionary: {:?}", dict);

    let tokens = lexer(&dict)
        .parse(script_src)
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], script_src));

    debug!("Tokens: {}", tokens.iter().map(|t| t.0.to_string()).join(" "));

    let declarations = expr_parser()
        .parse(
            tokens
                .as_slice()
                .map((script_src.len()..script_src.len()).into(), |(t, s)| (t, s)),
        )
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], script_src));

    //println!("Declarations: {:?}", declarations);

    let ledger_path = extract_ledger_path(&declarations, script_src, script_filepath).unwrap();

    let signals_path = extract_signals_path(&declarations, script_src, script_filepath);

    info!("Start resolving with ledger={:?}, signals={:?}", ledger_path, signals_path);

    let ledger = read_ledger(&ledger_path).expect("Failed to read ledger file");

    let src_dict = ledger.get_src_dict();

    info!("SrcId dictionary from ledger: {:?}", src_dict);

    let rules = resolve_ast(&declarations, &src_dict, &trie).expect("Failed to resolve AST into rules");

    info!("Finished resolving with Rules: {:#?}",
        rules.iter().map(|(key, _val)| key.to_string()).collect::<Vec<_>>()
    );

    if let Some(signals_path) = signals_path {
        let mut uid_hist = HashMap::new();
        let signals = read_csv(&signals_path).expect("Failed to read signals file");
        for signal in signals {
            *uid_hist.entry(signal.uid).or_insert(0) += 1;
        }
        // Get top most common UIDs
        let mut uid_hist_vec: Vec<_> = uid_hist.iter().collect();
        uid_hist_vec.sort_by_key(|&(_uid, count)| *count);

        let uids_top_10 = uid_hist_vec.iter().rev().take(10).map(|(uid, _count)| Uid::decode(**uid)).collect::<Vec<_>>();
        println!("Top 10 most common UIDs in signals: {:#?}", uids_top_10);
        let ledger_dirname = ledger_path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let dot_file = ledger_dirname.join(format!("{}_top.dot", script_filepath.file_stem().unwrap().to_str().unwrap()));
        let graphviz_dot = ledger.emit_dot(&uids_top_10);
        std::fs::write(&dot_file, graphviz_dot).expect("Failed to write DOT file");
    }

    for (rule_name, rule) in rules.iter() {
        println!("Rule: {rule_name}");
        let uids = find_forward_uid_rule(&ledger, rule);
        println!("Found UIDS: {:#?}", uids);

        let ledger_dirname = ledger_path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let dot_file = ledger_dirname.join(format!("{}_{}.dot", script_filepath.file_stem().unwrap().to_str().unwrap(), rule_name));
        let graphviz_dot = ledger.emit_dot(&uids);
        std::fs::write(&dot_file, graphviz_dot).expect("Failed to write DOT file");
    }
}
