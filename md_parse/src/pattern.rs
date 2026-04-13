use std::collections::{HashMap, HashSet};

use log::error;

use crate::{bits::BitsMatch, trie::{self, TrieNode}};

#[derive(Debug)]
pub enum Field<'a> {
    X,
    Field(&'a str),
}

#[derive(Debug)]
pub struct Pattern<'a>(pub Vec<Field<'a>>);

pub fn field_matches(pattern_field: &Field, trie_field: &trie::Field, attrs_map: &HashMap<String, Vec<String>>) -> bool {
    //println!("Matching pattern field {:?} with trie field {:?}", pattern_field, trie_field);
    match (pattern_field, trie_field) {
        (Field::X, _) => true, // 'X' matches any field
        (_, trie::Field::X { .. }) => true, // Specific field can match 'X' in trie
        (Field::Field(name), trie::Field::Named { name: trie_name, attr, .. }) => {
            if let Some(attr) = attr {
                attrs_map.get(attr)
                    .is_some_and(|values| values.contains(&name.to_string()))
            } else {
                name == trie_name
            }
        },
    }
}

pub fn search_trie(trie_node: &TrieNode, pattern: &Pattern, attrs_map: &HashMap<String, Vec<String>>) -> BitsMatch {
    let mut encodings = Vec::new();
    // FIXME: bits_match must hold bits range and attribute
    struct TrieField<'a> {
        node: &'a TrieNode,
        hold_x: bool,
    }
    #[derive(Clone, Hash, PartialEq, Eq)]
    struct StackEntry<'a> {
        node: &'a TrieNode,
        pattern_index: usize,
        prev_trie_x: bool,
        bits_match: BitsMatch,
        encoding: Vec<String>,
    }
    impl<'a> StackEntry<'a> {
        pub fn step_pattern(&self) -> StackEntry<'a> {
            StackEntry {
                node: &self.node,
                pattern_index: self.pattern_index + 1,
                prev_trie_x: self.prev_trie_x,
                bits_match: self.bits_match.clone(),
                encoding: self.encoding.clone(),
            }
        }
        pub fn step_trie(&self, child_field: &trie::Field, child_node: &'a trie::TrieNode) -> StackEntry<'a> {
            StackEntry {
                node: &child_node,
                pattern_index: self.pattern_index,
                prev_trie_x: matches!(child_field, trie::Field::X { attr:None, .. }),
                bits_match: self.bits_match.combine(child_field.bits_match()),
                encoding: {
                    let mut encoding = self.encoding.clone();
                    encoding.push(format!("{:?} ", child_field));
                    encoding
                },
            }
        }
    }
    struct Stack<'a> {
        stack: Vec<StackEntry<'a>>,
        visited: HashSet<StackEntry<'a>>,
    }
    impl<'a> Stack<'a> {
        pub fn new() -> Self {
            Stack {
                stack: Vec::new(),
                visited: HashSet::new(),
            }
        }
        pub fn push(&mut self, entry: StackEntry<'a>) {
            if self.visited.insert(entry.clone()) {
                self.stack.push(entry);
            }
        }
        pub fn pop(&mut self) -> Option<StackEntry<'a>> {
            self.stack.pop()
        }
    }
    // TODO: Memoize stack hash, to avoid checking same sequence from different combinations
    let mut stack = Stack::new();
    stack.push(StackEntry {
        // (Node, prev_trie_x) represent the trie node to match against
        // Marching through the Trie, we can either:
        // next(node, !prev_x) -> (next(node), node==x)
        // next(node, prev_x)  -> (next(node), node==x)
        //                     -> (node, prev_x)
        node: trie_node,
        prev_trie_x: false,
        // pattern[idx] is the field that tries to consume the Trie
        pattern_index: 0,
        // Accumulated BitsMatch while walking through the Trie
        bits_match: BitsMatch { mask: 0, value: 0 },
        // Encodings extracted from the Trie
        encoding: Vec::new(),
    });

    while let Some(entry) = stack.pop() {
        if entry.pattern_index == pattern.0.len() && entry.node.is_terminal {
            println!("Found match with encoding: {:#?}", entry.encoding);
            encodings.push(entry.bits_match);
            continue;
        } else if entry.pattern_index >= pattern.0.len() || entry.node.is_terminal {
            // Pattern is fully matched but trie path is not terminal,
            // or pattern is not fully matched but trie path is terminal
            continue;
        }

        let pattern_field = &pattern.0[entry.pattern_index];

        let mut check_prev_x = true;

        for (child_field, child_node) in &entry.node.children {
            match (pattern_field, child_field, entry.prev_trie_x) {
                (Field::Field(name), trie::Field::Named { name: child_name, .. }, _) => {
                    if name == child_name {
                        stack.push(entry.step_trie(&child_field, &child_node).step_pattern());
                        check_prev_x = false; // Match implies barrier on the X in the Trie
                    }
                }
                (Field::Field(name), trie::Field::X { attr: Some(attr), .. }, _) => {
                    if attrs_map
                        .get(attr)
                        .is_some_and(|values| values.contains(&name.to_string()))
                    {
                        stack.push(entry.step_trie(&child_field, &child_node).step_pattern());
                        check_prev_x = false; // Match implies barrier on the X in the Trie
                    }
                }
                (Field::Field(_name), trie::Field::X { .. }, _prev_x) => {
                    stack.push(entry.step_trie(&child_field, &child_node).step_pattern());
                }
                (Field::X, trie::Field::Named { .. }, _) => {
                    stack.push(entry.step_trie(&child_field, &child_node));
                    stack.push(entry.step_trie(&child_field, &child_node).step_pattern());
                }
                (Field::X, trie::Field::X { .. }, _prev_x) => {
                    stack.push(entry.step_trie(&child_field, &child_node));
                    stack.push(entry.step_trie(&child_field, &child_node).step_pattern());
                }
            }
        }

        if check_prev_x && entry.prev_trie_x {
            // NOTE: Avoid infinite recursion
            // stack.push(entry);
            stack.push(entry.step_pattern());
        }
    }

    println!("Found {} encodings matching pattern: {:#?}", encodings.len(), encodings);
    // Combine all encodings into one BitsMatch with mask covering all bits and value being the OR of all values
    // FIXME: Replace with combination instead of retruning first match
    encodings[0].clone()
}
