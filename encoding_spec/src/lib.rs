use std::{fmt::Display, str::FromStr};

use anyhow::{Error, anyhow, Result};

use crate::{parser::{parse_encodings, resolved_dir_encodings}, trie::Trie};

pub mod parser;
pub mod trie;
pub mod bits;
pub mod pattern;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum SrcId {
    SrcId,
    MatSurfId,
    MatId,
    SurfId,
    LightId,
    DetectorId,
}

impl FromStr for SrcId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SrcId" => Ok(SrcId::SrcId),
            "MatSurfId" => Ok(SrcId::MatSurfId),
            "MatId" => Ok(SrcId::MatId),
            "SurfId" => Ok(SrcId::SurfId),
            "LightId" => Ok(SrcId::LightId),
            "DetectorId" | "DetId" => Ok(SrcId::DetectorId),
            _ => Err(anyhow!("Unknown SrcId type: {}", s)),
        }
    }
}

impl Display for SrcId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SrcId::SrcId      => write!(f, "SrcId::SrcId"),
            SrcId::MatSurfId  => write!(f, "SrcId::MatSurfId"),
            SrcId::MatId      => write!(f, "SrcId::MatId"),
            SrcId::SurfId     => write!(f, "SrcId::SurfId"),
            SrcId::LightId    => write!(f, "SrcId::LightId"),
            SrcId::DetectorId => write!(f, "SrcId::DetId"),
        }
    }
}

impl SrcId {
    pub fn combine(&self, other: &SrcId) -> Option<SrcId> {
        Some(match (self, other) {
            (SrcId::SrcId,      SrcId::MatSurfId ) => SrcId::MatSurfId,
            (SrcId::SrcId,      SrcId::MatId     ) => SrcId::MatId,
            (SrcId::SrcId,      SrcId::SurfId    ) => SrcId::MatId,
            (SrcId::SrcId,      SrcId::LightId   ) => SrcId::LightId,
            (SrcId::SrcId,      SrcId::DetectorId) => SrcId::DetectorId,
            (SrcId::MatSurfId,  SrcId::MatId     ) => SrcId::MatId,
            (SrcId::MatSurfId,  SrcId::SurfId    ) => SrcId::SurfId,
            // The same but in reverse
            (SrcId::MatSurfId,  SrcId::SrcId     ) => SrcId::MatSurfId,
            (SrcId::MatId,      SrcId::SrcId     ) => SrcId::MatId,
            (SrcId::SurfId,     SrcId::SrcId     ) => SrcId::MatId,
            (SrcId::LightId,    SrcId::SrcId     ) => SrcId::LightId,
            (SrcId::DetectorId, SrcId::SrcId     ) => SrcId::DetectorId,
            (SrcId::MatId,      SrcId::MatSurfId ) => SrcId::MatId,
            (SrcId::SurfId,     SrcId::MatSurfId ) => SrcId::SurfId,
            (lhs, rhs) if lhs == rhs => lhs.clone(),
            _ => return None,
        })
    }
}

pub fn build_decoder(src: &str) -> Result<Trie> {
    let encodings = parse_encodings(src)?;
    let dir_encodings = resolved_dir_encodings(&encodings);
    //println!("Parsed encodings: {:#?}", encodings);

    let mut trie = Trie::new();
    for encoding in encodings {
        trie.insert(&encoding);
    }

    for encoding in dir_encodings {
        trie.insert(&encoding);
    }

    let graphviz_dot = trie.emit_dot();
    std::fs::write("decoder.dot", graphviz_dot).expect("Failed to write DOT file");

    Ok(trie)
}
