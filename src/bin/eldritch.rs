use aetherus_events::{
    ledger::Uid,
    reader::{read_csv, read_ledger},
};
use anyhow::Result;
use env_logger::Env;
use filter_dsl::{extract_ledger_path, extract_signals_path, model::resolve_ast, parse_script};
use std::{collections::HashMap, env, fs, path::Path};

use log::info;

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let encoding_filename = env::args().nth(1).expect("Expected file argument for encoding scheme");
    let encoding_src = &fs::read_to_string(&encoding_filename).expect("Failed to read encoding scheme file");

    let script_arg = env::args().nth(2).expect("Expected file argument for DSL script");
    let script_filepath = Path::new(&script_arg);
    let script_src = &fs::read_to_string(&script_filepath).expect("Failed to read script file");

    let trie = encoding_spec::build_decoder(encoding_src)
        .expect("Failed to build decoder from encoding scheme");

    let dict = trie.get_fields();
    info!("FieldId dictionary: {:?}", dict);

    let declarations = parse_script(script_src, &dict);

    //println!("Declarations: {:?}", declarations);

    let ledger_path = extract_ledger_path(&declarations, script_src, script_filepath).unwrap();

    let signals_path = extract_signals_path(&declarations, script_src, script_filepath);

    info!(
        "Start resolving with ledger={:?}, signals={:?}",
        ledger_path, signals_path
    );

    let ledger = read_ledger(&ledger_path).expect("Failed to read ledger file");

    let src_dict = ledger.get_src_dict();

    info!("SrcId dictionary from ledger: {:?}", src_dict);

    let rules = resolve_ast(&script_src, &declarations, &src_dict, &trie);

    info!(
        "Finished resolving with Rules: {:#?}",
        rules
            .iter()
            .map(|(key, _val)| key.to_string())
            .collect::<Vec<_>>()
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

        let uids_top_10 = uid_hist_vec
            .iter()
            .rev()
            .take(10)
            .map(|(uid, _count)| Uid::decode(**uid))
            .collect::<Vec<_>>();
        println!("Top 10 most common UIDs in signals: {:#?}", uids_top_10);
        let ledger_dirname = ledger_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let dot_file = ledger_dirname.join(format!(
            "{}_top.dot",
            script_filepath.file_stem().unwrap().to_str().unwrap()
        ));
        let graphviz_dot = ledger.emit_dot(&uids_top_10);
        std::fs::write(&dot_file, graphviz_dot).expect("Failed to write DOT file");
    }

    for (rule_name, rule) in rules.iter() {
        println!("Rule: {rule_name}");
        let uids = rule.evaluate(&ledger)?;
        println!("Found UIDS: {:#?}", uids);

        let ledger_dirname = ledger_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let dot_file = ledger_dirname.join(format!(
            "{}_{}.dot",
            script_filepath.file_stem().unwrap().to_str().unwrap(),
            rule_name
        ));
        let graphviz_dot = ledger.emit_dot(&uids);
        std::fs::write(&dot_file, graphviz_dot).expect("Failed to write DOT file");
    }

    Ok(())
}
