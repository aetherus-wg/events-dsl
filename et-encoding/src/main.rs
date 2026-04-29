//! This binary demonstrates how to read encoding schemes from a file, build a Trie decoder, emit the DOT file to visualiset the Trie, and perform pattern matching on the Trie.

use anyhow::{Result};
use et_encoding::SrcId;
use et_encoding::parser::{parse_encodings, resolved_dir_encodings};
use et_encoding::pattern::{self, Pattern};
use et_encoding::trie::Trie;
use std::env;
use std::fs;

/// This binary reads an encoding scheme from a file, builds a Trie decoder, and demonstrates pattern matching on the Trie.
fn main() -> Result<()> {
    env_logger::init();

    let filename = env::args().nth(1).expect("Expected file argument");
    let src = &fs::read_to_string(&filename).expect("Failed to read file");

    let encodings = parse_encodings(src)?;
    let dir_encodings = resolved_dir_encodings(&encodings);
    //println!("Parsed encodings: {:#?}", encodings);

    let mut trie = Trie::new();
    for encoding in encodings {
        trie.insert(&encoding);
    }

    let dot_file = filename.replace(".md", ".dot");
    let graphviz_dot = trie.emit_dot();
    std::fs::write(&dot_file, graphviz_dot).expect("Failed to write DOT file");

    for encoding in dir_encodings {
        trie.insert(&encoding);
    }

    let dot_file = filename.replace(".md", "_complete.dot");
    let graphviz_dot = trie.emit_dot();
    std::fs::write(&dot_file, graphviz_dot).expect("Failed to write DOT file");

    let pattern = Pattern(vec![
        pattern::Field::Field("MCRT"),
        pattern::Field::Field("Material"),
        pattern::Field::Field("Elastic"),
        //pattern::Field::Field("Mie"),
        pattern::Field::X,
        pattern::Field::Field("Forward"),
        //pattern::Field::X,
        pattern::Field::SrcId(SrcId::MatId),
    ]);

    let (bits_match, src_id) = pattern::search_trie(&trie, &pattern)?;

    println!("Bits match for pattern {:?}: {:?} with {}", pattern, bits_match, src_id);

    Ok(())
}
