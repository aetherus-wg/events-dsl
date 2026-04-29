//! Encoding parser and single source of truth provider
//!
//! This library parses the 32-bit encoding of the events,
//! described in markdown table as ranges of bits with names, values and optional attributes.
//!
//! Each field might be any of the following
//!  - Named field with a specific value (or __partial__ value)
//!  - Don't care ("X" or "_") - matches any value
//!  - Reserved field ("_<reserved_field_name") which is ignored in building the encodign Trie.
//!
//! Once parsed, the encoding names and values are organised in a Trie with all possible matching
//! described, including X values. The Trie is used to generate the BitsMatch condition for an
//! encoding to match with the pattern provided(pattern = list of named fields concatenated)
//!
//! Parsing the encoding specification, also provides us with a list of all named fields,
//! which are used as the dictionary for parsing the pattern described in the script with the et-dsl crate.

use std::{fmt::Display, str::FromStr};

use anyhow::{Error, Result, anyhow};

use crate::{
    parser::{parse_encodings, resolved_dir_encodings},
    trie::Trie,
};

pub mod bits;
pub mod parser;
pub mod pattern;
pub mod trie;

/// This enum represents the different types of source identifiers that can be used in the encoding patterns.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum SrcId {
    /// A generic source identifier representing the abstract SrcId
    SrcId,
    /// Union of MatId and SurfId, generally used to describe a transparent object
    MatSurfId,
    /// Material identifier type
    MatId,
    /// Surface identifier type
    SurfId,
    /// Light/Emitter identifier type
    LightId,
    /// Detector identifier type
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
    /// Combine two SrcId types to return most restrictive type,
    /// or None if the types are on disjoint branches of the type hierarchy.
    /// Functionality is identical to Julia type multiple dispatch,
    /// choosing most restrictive type.
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

/// Builds a Trie decoder from the given encoding specification string.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srcid_from_str_valid() {
        assert_eq!(SrcId::from_str("SrcId").unwrap(), SrcId::SrcId);
        assert_eq!(SrcId::from_str("MatSurfId").unwrap(), SrcId::MatSurfId);
        assert_eq!(SrcId::from_str("MatId").unwrap(), SrcId::MatId);
        assert_eq!(SrcId::from_str("SurfId").unwrap(), SrcId::SurfId);
        assert_eq!(SrcId::from_str("LightId").unwrap(), SrcId::LightId);
        assert_eq!(SrcId::from_str("DetectorId").unwrap(), SrcId::DetectorId);
        assert_eq!(SrcId::from_str("DetId").unwrap(), SrcId::DetectorId);
    }

    #[test]
    fn srcid_from_str_invalid() {
        assert!(SrcId::from_str("InvalidType").is_err());
        assert!(SrcId::from_str("").is_err());
        assert!(SrcId::from_str("SRCID").is_err());
    }

    #[test]
    fn srcid_combine_src_id() {
        let result = SrcId::SrcId.combine(&SrcId::MatSurfId);
        assert_eq!(result, Some(SrcId::MatSurfId));

        let result = SrcId::SrcId.combine(&SrcId::MatId);
        assert_eq!(result, Some(SrcId::MatId));

        let result = SrcId::SrcId.combine(&SrcId::LightId);
        assert_eq!(result, Some(SrcId::LightId));

        let result = SrcId::SrcId.combine(&SrcId::DetectorId);
        assert_eq!(result, Some(SrcId::DetectorId));
    }

    #[test]
    fn srcid_combine_reversed() {
        let result = SrcId::MatSurfId.combine(&SrcId::SrcId);
        assert_eq!(result, Some(SrcId::MatSurfId));

        let result = SrcId::MatId.combine(&SrcId::SrcId);
        assert_eq!(result, Some(SrcId::MatId));

        let result = SrcId::SurfId.combine(&SrcId::SrcId);
        assert_eq!(result, Some(SrcId::MatId));
    }

    #[test]
    fn srcid_combine_same_type() {
        let result = SrcId::MatSurfId.combine(&SrcId::MatSurfId);
        assert_eq!(result, Some(SrcId::MatSurfId));

        let result = SrcId::LightId.combine(&SrcId::LightId);
        assert_eq!(result, Some(SrcId::LightId));
    }

    #[test]
    fn srcid_combine_incompatible() {
        let result = SrcId::LightId.combine(&SrcId::MatId);
        assert_eq!(result, None);

        let result = SrcId::DetectorId.combine(&SrcId::LightId);
        assert_eq!(result, None);

        let result = SrcId::MatId.combine(&SrcId::LightId);
        assert_eq!(result, None);
    }

    #[test]
    fn srcid_combine_mat_surf_id_with_mat() {
        let result = SrcId::MatSurfId.combine(&SrcId::MatId);
        assert_eq!(result, Some(SrcId::MatId));
    }

    #[test]
    fn srcid_combine_mat_surf_id_with_surf() {
        let result = SrcId::MatSurfId.combine(&SrcId::SurfId);
        assert_eq!(result, Some(SrcId::SurfId));
    }
}
