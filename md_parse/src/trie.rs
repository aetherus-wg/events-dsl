use crate::{SrcId, bits::BitsMatch};
use std::str::FromStr;
use anyhow::{anyhow, Error};

#[derive(Debug, Clone, Hash)]
pub enum Field {
    X {
        attr: Option<String>,
        size: usize
    },
    SrcId(SrcId),
    Named {
        name: String,
        attr: Option<String>,
        bits: BitsMatch,
        size: usize,
    },
}

impl Field {
    pub fn bits_match(&self) -> &BitsMatch {
        match self {
            Field::X { .. }           => &BitsMatch { mask: 0, value: 0 },
            Field::SrcId(_)           => &BitsMatch { mask: 0, value: 0 },
            Field::Named { bits, .. } => bits,
        }
    }
    pub fn size(&self) -> usize {
        match self {
            Field::X { size, .. }     => *size,
            Field::SrcId(_)           => 16,
            Field::Named { size, .. } => *size,
        }
    }
}

impl FromStr for Field {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(src_id) = SrcId::from_str(s) {
            Ok(Field::SrcId(src_id))
        } else {
            Err(anyhow!("Unknown field type: {}", s))
        }
    }
}

impl std::fmt::Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Field::X{size, attr: Some(attr)}            => write!(f, "X({size} bits)\n{{{attr}}}"),
            Field::X{size, ..}                          => write!(f, "X({size} bits)"),
            Field::SrcId(src_id)                        => write!(f, "{src_id}"),
            Field::Named { name, attr: Some(attr), .. } => write!(f, "{attr}::{name}"),
            Field::Named { name, .. }                   => write!(f, "{}", name),
        }
    }
}

impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Field::X{size: bits1, ..}, Field::X{size: bits2, ..}) => bits1 == bits2,
            (
                Field::Named {
                    name: name1,
                    attr: _,
                    bits: bits1,
                    size: size1,
                },
                Field::Named {
                    name: name2,
                    attr: _,
                    bits: bits2,
                    size: size2,
                },
            ) => {
                // name1==name2 |-> bits1==bits2 and size1==size2
                assert!(
                    name1 != name2 || (bits1 == bits2 && size1 == size2),
                    "Field with the same name has different encodings"
                );
                name1 == name2
            }
            (Field::SrcId(src_id1), Field::SrcId(src_id2)) => src_id1 == src_id2,
            _ => false,
        }
    }
}
impl Eq for Field {}

#[derive(Debug)]
pub struct Encoding(pub Vec<Field>);

macro_rules! encoding {
    ($($field:ident, $bits:expr),* ,$src_id:ident) => {
        let lens = vec![$($bits),*].iter().map(|b| stringify!($src_id).split("0b")[1].len()).sum::<usize>();
        Encoding(vec![
            $(Field::Named(stringify!($field).to_string(), BitsMatch::parse(&$bits))),* ,
             Field::Named(stringify!($src_id).to_string(), BitsMatch::parse(&BitsRange(15, 0)))
        ])
    };
}

// FIXME: no need for public fields
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct TrieNode {
    pub is_terminal: bool,
    pub children: Vec<(Field, TrieNode)>,
}

#[derive(Debug)]
pub struct Trie {
    pub root: TrieNode,
}

impl Trie {
    /// Returns a new, empty Trie.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Insert an encoding into the trie
    ///
    /// # Examples
    ///
    /// ```
    /// use md_parse::Trie;
    /// let mut trie = Trie::new();
    ///
    /// let encoding = encoding!(MCRT, 0b0011, Material, 0b01, Elastic, 0b10, X, 0b0000, MatSurfId);
    ///
    /// trie.insert(encoding);
    /// assert_eq!(vec![BitsMatch{mask: 0x0FA00000, value: 0x0360000}], trie.get_all());
    /// ```
    pub fn insert(&mut self, encoding: &Encoding) {
        let mut current = &mut self.root;

        for field in &encoding.0 {
            if current.get_mut(field).is_none() {
                current.insert_new(field);
            }

            current = current.get_mut(field).unwrap();
        }

        current.is_terminal = true;
    }

    pub fn emit_dot(&self) -> String {
        fn walk(node: &TrieNode, id: usize, next_id: &mut usize, out: &mut String) {
            if node.is_terminal {
                out.push_str(&format!("n{id} [label=\"{}\", shape=doublecircle];\n", id));
            } else {
                out.push_str(&format!("n{id} [label=\"{}\", shape=circle];\n", id));
            }

            for (field, child) in &node.children {
                *next_id += 1;
                let child_id = *next_id;
                out.push_str(&format!("n{id} -> n{child_id} [label=\"{}\"];\n", field));
                walk(child, child_id, next_id, out);
            }
        }

        let mut out = String::from("digraph Trie {\nrankdir=LR;\nnode [fontname=\"Arial\"];\n");
        let mut next_id = 0;
        walk(&self.root, 0, &mut next_id, &mut out);
        out.push_str("}\n");
        out
    }

    /// Returns true if the trie contains 'query' as a partial encoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use basic_trie::Trie;
    /// let mut trie = Trie::new();
    ///
    /// let encoding = encoding!(MCRT, 0b0011, Material, 0b01, Elastic, 0b10, X, 0b0000, MatSurfId);
    ///
    /// trie.insert(encoding);
    /// assert!(trie.contains(vec![Some("MCRT"), Some("Material"), Some("Elastic"), None, Some("MatSurfId")]));
    /// assert!(!trie.contains(vec![Some("MCRT"), Some("Material"), Some("Elastic"), Some("MatSurfId")]));
    /// ```
    //pub fn contains(&self, query: Vec<Option<&str>>) -> bool {
    //    self.get_final_node(query)
    //        .is_some_and(|node| node.is_associated())
    //}

    /// Returns an option enum with a vector of owned strings
    /// representing all found words that begin with 'query'.
    /// If the word 'query' doesn't exist, None is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use basic_trie::Trie;
    /// let mut trie = Trie::new();
    ///
    /// trie.insert("word1");
    /// trie.insert("word2");
    ///
    /// let all_correct_words = vec![String::from("word1"), String::from("word2")];
    /// let mut found_words = trie.get("word").unwrap();
    /// found_words.sort();
    /// assert_eq!(all_correct_words, found_words);
    /// ```
    pub fn get(&self, query: Vec<Option<&str>>) -> Option<Vec<String>> {
        todo!()
    }
    //pub fn get(&self, query: Vec<Option<&str>>) -> Option<Vec<String>> {
    //    let mut substring = String::new();
    //    let mut current_node = &self.root;
    //    let characters = get_characters(query);

    //    for character in characters {
    //        current_node = match current_node.children.get(character) {
    //            None => return None,
    //            Some(trie_node) => {
    //                substring.push(character);
    //                trie_node
    //            }
    //        }
    //    }

    //    let mut words_vec = Vec::new();
    //    current_node.find_words(&mut substring, &mut words_vec);

    //    Some(words_vec)
    //}
}

impl TrieNode {
    /// Returns a new TrieNode with no children and is_end set to false.
    pub fn new() -> Self {
        Self {
            is_terminal: false,
            children: Vec::new(),
        }
    }

    pub fn get_mut(&mut self, field: &Field) -> Option<&mut TrieNode> {
        self.children
            .iter_mut()
            .find_map(|(f, node)| if f == field { Some(node) } else { None })
    }

    pub fn insert_new(&mut self, field: &Field) {
        self.children.push((field.clone(), TrieNode::new()));
    }
}
