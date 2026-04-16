use std::collections::HashMap;

use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};
use encoding_spec::{pattern::{self, Pattern}, trie::Trie, bits::BitsMatch};

use crate::ast::{Expr, Repetition, SrcId};

// -------------------------------------------------
// AST -> Semantics Model
// -------------------------------------------------

#[derive(Debug, Clone)]
pub enum Match {
    X,
    Yes(BitsMatch),
    No(Box<Match>),
    And(Box<Match>, Box<Match>),
    Any(Vec<Match>),
}

impl Into<encoding_spec::SrcId> for SrcId<'_> {
    fn into(self) -> encoding_spec::SrcId {
        match self {
            Self::None            => panic!("Cannot convert None SrcId to encoding_spec::SrcId"),
            Self::Mat(_)          => encoding_spec::SrcId::MatId,
            Self::Surf(_)         => encoding_spec::SrcId::SurfId,
            Self::MatSurf(_)      => encoding_spec::SrcId::MatSurfId,
            Self::Light(_)        => encoding_spec::SrcId::LightId,
            Self::Detector(_)     => encoding_spec::SrcId::DetectorId,
            Self::MatName(_)      => encoding_spec::SrcId::MatId,
            Self::SurfName(_)     => encoding_spec::SrcId::SurfId,
            Self::MatSurfName(_)  => encoding_spec::SrcId::MatSurfId,
            Self::LightName(_)    => encoding_spec::SrcId::LightId,
            Self::DetectorName(_) => encoding_spec::SrcId::DetectorId,
        }
    }
}

impl<'src> Expr<'src> {
    pub fn resolve_src(&self, src_dict: &HashMap<SrcName, DomainSrcId>) -> (encoding_spec::SrcId, Match) {
        match self {
            Self::X => (encoding_spec::SrcId::SrcId, Match::X),
            Self::SrcId(src_id) => (src_id.clone().into(), Match::Yes(src_id.resolve(src_dict).bits_match().into())),
            Self::Not(boxed) => {
                let (inner, span) = boxed.as_ref();
                let (src_id_type, src_id_match) = inner.resolve_src(src_dict);
                (src_id_type, Match::No(Box::new(src_id_match)))
            }
            // TODO: If error occur extract the span and propagate it back as Result
            // Perhaps make use of ariadne to properly show diagnostics relevant to code
            Self::Any(exprs) => {
                let src_id_matches: Vec<_> = exprs.iter().map(|e| e.0.resolve_src(src_dict)).collect();
                let src_id_type = src_id_matches.iter().fold(encoding_spec::SrcId::SrcId, |acc, (src_id, _)| {
                    acc.combine(&src_id).unwrap()
                });
                // TODO: flatten nested Any
                (src_id_type, Match::Any(src_id_matches.into_iter().map(|(_, r#match)| r#match).collect()))
            }
            _ => panic!("Unexpected expression type for resolving SrcId: {:?}", self),
        }
    }

    pub fn resolve_pattern(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<String, (encoding_spec::SrcId, Match)>
    ) -> Match {
        match self {
            Self::X => Match::X,
            Self::Pattern(fields) => {
                let src_field = &fields
                    .last()
                    .unwrap_or_else(|| panic!("Pattern must have at least one field (the SrcId)")).0;
                let (src_id_type, src_id_match) = match src_field {
                    Expr::SrcId(_) | Expr::Any(_) => {
                        src_field.resolve_src(src_dict)
                    }
                    Expr::Ident(src_ident) => {
                        resolved_src.get(*src_ident)
                            .cloned()
                            .unwrap_or_else(|| panic!("Unknown source identifier in pattern: {}", src_ident))
                    }
                    _ => panic!("Unexpected expression type for SrcId in pattern: {:?}", src_field),
                };

                let mut fields: Vec<_> = fields[..fields.len() - 1]
                    .iter()
                    .map(|(field, span)| match field {
                        Expr::X => pattern::Field::X,
                        Expr::Field(field_id) => pattern::Field::Field(field_id),
                        _ => panic!("Unexpected expression type in pattern fields: {:?} at {}", field, span),
                    })
                    .collect();

                fields.push(pattern::Field::SrcId(src_id_type));
                let pattern = Pattern(fields);
                let (bits_match, src_id) = trie.get(&pattern);
                Match::And(Box::new(Match::Yes(bits_match)), Box::new(src_id_match))
            }
            Self::Any(exprs) => Match::Any(exprs.iter().map(|(e, span)| e.resolve_pattern(src_dict, trie, resolved_src)).collect()),
            _ => panic!("Unexpected expression type for resolving SrcId: {:?}", self),
        }
    }

    pub fn resolve_seq(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<String, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<String, Match>
    ) -> SeqTree {
        todo!()
    }

    pub fn resolve_rule(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<String, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<String, Match>,
        resolved_seq: &HashMap<String, SeqTree>,
    ) -> Rule {
        todo!()
    }
}

#[derive(Debug)]
pub enum Value<'src, T>
{
    X,
    Ident(&'src str),
    Primitive(T),
    // NOTE: There is no reason to support nested "any" since it trivially flattens
    Any(Vec<Self>),
    Not(Box<Self>),
}

// Sequence Tree
#[derive(Debug)]
pub enum SeqTree {
    Primitive(Repetition, Match),
    Any(Match),
    // TODO: Enable permutations with splatting (Julia nomenclature)
    Perm(Vec<Self>),
    Seq(Vec<Self>),
}

#[derive(Debug)]
pub enum Condition {
    Pattern(Repetition, Match),
    Seq(SeqTree),
}

#[derive(Debug)]
pub struct Rule<'src> {
    name: &'src str,
    conditions: Vec<Condition>,
}
