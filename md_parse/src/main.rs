use anyhow::anyhow;
use anyhow::{Error, Result};
use markdown_ppp::ast::Block;
use markdown_ppp::ast::{Document, Inline};
use markdown_ppp::parser::MarkdownParserState;
use markdown_ppp::parser::parse_markdown;
use md_parse::SrcId;
use md_parse::bits::{BitsRange, BitsMatch};
use md_parse::trie::{Encoding, Field, Trie};
use md_parse::pattern::{self, Pattern};
use std::collections::HashMap;
use std::env;
use std::fs;
use itertools::Itertools;
use std::str::FromStr;

fn unwrap_inline(inline: &Inline) -> Result<String> {
    match inline {
        Inline::Text(s) => Ok(s.clone()),
        _ => Err(anyhow!("Expected text, received {:?}", inline)),
    }
}

fn parse_encodings(src: &str) -> Result<Vec<Encoding>> {
    let state = MarkdownParserState::new();
    let document = parse_markdown(state, src).unwrap();

    let mut encodings = Vec::new();

    for block in document.blocks {
        if let Block::Table(table) = block {
            let bits_ranges: Vec<BitsRange> = table.rows[0]
                .iter()
                .map(|c| match &c[0] {
                    Inline::Text(s) => BitsRange::from_str(&s),
                    _ => Err(anyhow!("Expected text in header cell, received: {:?}", c[0])),
                })
                .collect::<Result<Vec<_>, _>>()?; // header row

            for (fields_md, encodings_md) in table.rows.iter().skip(1).tuples() {
                let mut specified = true;
                let mut fields_encoding = Vec::new();
                for (bit_range, (field_md, encoding_md)) in
                    bits_ranges.iter().zip(fields_md.iter().zip(encodings_md.iter()))
                {
                    let field_str = unwrap_inline(&field_md[0])?;
                    let encoding = BitsMatch::parse(&bit_range, &unwrap_inline(&encoding_md[0])?);
                    if field_str.starts_with('_') && field_str.len() > 1 {
                        specified = false;
                    }

                    // TODO: Regex parse to extract name and optional attr from
                    // "<name> {<attr>}" or "<name>{<attr>}"
                    let re = regex::Regex::new(r"(\w*)(?:\s*\{([^}]*)\})?").unwrap();
                    let field = if let Some(caps) = re.captures(&field_str) {
                        let name = caps.get(1).unwrap().as_str().to_string();
                        let attr = caps.get(2).map(|m| m.as_str().to_string());
                        if attr == Some("SrcId".to_string()) {
                            Field::from_str(name.as_str())?
                        }
                        else if name == "_" || name == "" || name == "X" {
                            Field::X{size: bit_range.size(), attr}
                        } else {
                            Field::Named {
                                name,
                                attr,
                                bits: encoding.clone(),
                                size: bit_range.size(),
                            }
                        }
                    } else {
                        panic!("Invalid field name format: {}", field_str);
                    };
                    fields_encoding.push(field);
                }
                if specified {
                    encodings.push(Encoding(fields_encoding));
                }
            }
        }
    }
    Ok(encodings)
}

fn resolved_dir_encodings(encodings: &Vec<Encoding>) -> Vec<Encoding> {
    let dir_fields: Vec<Field> = encodings
        .iter()
        .map(|Encoding(fields_encoding)| fields_encoding)
        .filter_map(|fields_encoding|
            fields_encoding.iter().find(|&field|
                matches!(field, Field::Named{name, attr, bits: _, size: _} if attr.as_deref() == Some("Direction"))).cloned()
        )
        .collect();
    encodings
        .iter()
        .flat_map(|Encoding(fields_encoding)| {
            fields_encoding
                .iter()
                .enumerate()
                .filter_map(|(pos, field)|
                    matches!(
                        field,
                        Field::X {attr, size: field_size} if attr.as_deref() == Some("Direction")
                    )
                    .then_some(pos)
                )
                .flat_map({
                    let dir_fields_captured = dir_fields.clone();
                    move |pos| {
                    dir_fields_captured
                            .clone()
                            .into_iter()
                            .map(move |dir_field| {
                                assert_eq!(dir_field.size(), fields_encoding[pos].size());
                                let mut new_encoding = fields_encoding.clone();
                                new_encoding[pos] = dir_field.clone();
                                Encoding(new_encoding)
                            })
                }})
        })
        .collect()
    //if let Some(pos) = fields_encoding
    //    .iter()
    //    .position(|field| matches!(
    //        field,
    //        Field::X{ attr, size: _} if attr.as_deref() == Some("Direction")
    //    ))
    //{
    //    let mut new_encoding = fields_encoding.clone();
    //    new_encoding[pos] = Field::Named { name: "Forward".to_string(), attr: Some("Direction".to_string()), bits: BitsMatch{mask: 0, value: 0}, size: 2};
    //    encodings.push(Encoding(new_encoding));
    //}
}

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

    let attrs_map = HashMap::from([
        (
            "Direction".to_string(),
            vec!["Forward".to_string(),
                 "Backward".to_string(),
                 "Side".to_string(),
                 "Unknown".to_string(),
            ]
        ),
    ]);

    let (bits_match, src_id) = pattern::search_trie(&trie.root, &pattern, &attrs_map);

    println!("Bits match for pattern {:?}: {:?} with {}", pattern, bits_match, src_id);

    Ok(())
}
