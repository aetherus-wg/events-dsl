use std::collections::HashMap;

use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};
use anyhow::{Context, Result, anyhow};
use encoding_spec::{pattern::{self, Pattern}, trie::Trie, bits::BitsMatch};
use log::{debug, info};

use crate::ast::{DeclType, Declaration, Expr, Repetition, SrcId};

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

impl Match {
    pub fn optimise(self) -> Self {
        match self {
            Match::And(ref left, ref right) => match (left.as_ref(), right.as_ref()) {
                (Match::Yes(bits1), Match::Yes(bits2)) => Match::Yes(bits1.combine(&bits2)),
                (Match::X, other) | (other, Match::X) => other.clone().optimise(),
                _ => self,
            }
            Match::Any(matches) => {
                assert!(!matches.is_empty(), "Any match should have at least one inner match");

                let mut flattened = Vec::new();
                for m in matches.into_iter() {
                    match m.optimise() {
                        Match::Any(inner) => flattened.extend(inner),
                        other => flattened.push(other),
                    }
                }

                if flattened.len() == 1 {
                    flattened[0].to_owned()
                } else {
                    for m in &flattened {
                        if matches!(m, Match::X) {
                            return Match::X;
                        }
                    }
                    // TODO: Optimise logic operations found for x86
                    // to evaluate an u32 value with Match::Any
                    //   - SAT solver or some for of boolean expression optimisation
                    //   - Construct tree with minimum depth that pairs overlapping masks
                    Match::Any(flattened)
                }
            }
            other => other,
        }
    }
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
    pub fn resolve_src(&self, src_dict: &HashMap<SrcName, DomainSrcId>) -> Result<(encoding_spec::SrcId, Match)> {
        Ok(
        match self {
            Self::X => (encoding_spec::SrcId::SrcId, Match::X),
            Self::SrcId(src_id) => {
                debug!("Resolving source identifier: {:?}", src_id);
                let src_bits_match = src_id.resolve(src_dict)?.bits_match().into();
                (src_id.clone().into(), Match::Yes(src_bits_match))
            },
            Self::Not(boxed) => {
                let (inner, span) = boxed.as_ref();
                let (src_id_type, src_id_match) = inner.resolve_src(src_dict)?;
                (src_id_type, Match::No(Box::new(src_id_match)))
            }
            // TODO: If error occur extract the span and propagate it back as Result
            // Perhaps make use of ariadne to properly show diagnostics relevant to code
            Self::Any(exprs) => {
                let src_id_matches: Vec<_> = exprs.iter().map(|e| e.0.resolve_src(src_dict)).collect::<Result<_,_>>()?;
                let src_id_type = src_id_matches.iter().fold(encoding_spec::SrcId::SrcId, |acc, (src_id, _)| {
                    acc.combine(&src_id).unwrap()
                });
                let src_id_match = Match::Any(src_id_matches.into_iter().map(|(_, r#match)| r#match).collect());
                debug!("Combined any SrcId types into: ({:?}, {:?})", src_id_type, src_id_match);
                // TODO: flatten nested Any
                (src_id_type, src_id_match.optimise())
            }
            _ => return Err(anyhow!("Unexpected expression type for resolving SrcId: {:?}", self)),
        }
        )
    }

    pub fn resolve_pattern(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>
    ) -> Result<Match> {
        Ok(match self {
            Self::X => Match::X,
            Self::Pattern(fields) => {
                let src_field = match &fields.last() {
                    Some(field) => &field.0,
                    // TODO: Include span information
                    None => return Err(anyhow!("Pattern must have at least one field (the SrcId)")),
                };
                let (src_id_type, src_id_match) = match src_field {
                    Expr::SrcId(_) | Expr::Any(_) => {
                        src_field.resolve_src(src_dict)?
                    }
                    Expr::Ident(src_ident) => {
                        match resolved_src.get(*src_ident) {
                            Some(ident) => ident.clone(),
                            // TODO: Include span information
                            None => return Err(anyhow!("Unknown source identifier in pattern: {}", src_ident)),
                        }
                    }
                    _ => return Err(anyhow!("Unexpected expression type for SrcId in pattern: {:?}", src_field)),
                };

                let mut fields: Vec<_> = fields[..fields.len() - 1]
                    .iter()
                    .map(|(field, span)| match field {
                        Expr::X => Ok(pattern::Field::X),
                        Expr::Field(field_id) => Ok(pattern::Field::Field(field_id)),
                        _ => Err(anyhow!("Unexpected expression type in pattern fields: {:?} at {}", field, span)),
                    })
                    .collect::<Result<_>>()?;

                fields.push(pattern::Field::SrcId(src_id_type));
                let pattern = Pattern(fields);
                let (bits_match, _src_id_type) = trie.get(&pattern);
                Match::And(Box::new(Match::Yes(bits_match)), Box::new(src_id_match)).optimise()
            }
            Self::Any(exprs) => {
                let bits_match = exprs.iter().map(|(e, span)| e.resolve_pattern(src_dict, trie, resolved_src)).collect::<Result<_>>()?;
                Match::Any(bits_match).optimise()
            },
            _ => return Err(anyhow!("Unexpected expression type for resolving SrcId: {:?}", self)),
        })
    }

    pub fn resolve_seq(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<&'src str, Match>
    ) -> Result<SeqTree> {
        Ok(match self {
            Self::X => SeqTree::Pattern(Match::X),
            Self::Seq(exprs) => SeqTree::Seq(exprs.iter().map(|(e, span)| e.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)).collect::<Result<_>>()?),
            Self::Perm(_exprs) => todo!("Permutation feature not implemented yet"),
            //Self::Perm(exprs) => SeqTree::Perm(exprs.iter().map(|(e, span)| e.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)).collect()?),
            Self::Any(exprs) => SeqTree::Pattern(Match::Any(exprs.iter().map(|(e, span)| e.resolve_pattern(src_dict, trie, resolved_src)).collect::<Result<_>>()?).optimise()),
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                SeqTree::Repeat(repetition.clone(), Box::new(pattern.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?))
            },
            Self::Ident(pattern_name) => {
                match resolved_pattern.get(*pattern_name) {
                    Some(pattern_match) => SeqTree::Pattern(pattern_match.clone()),
                    None => return Err(anyhow!("Unknown pattern in sequence: {}", pattern_name)),
                }
            }
            Self::Pattern(_expr) => {
                let pattern_match = self.resolve_pattern(src_dict, trie, resolved_src)?;
                SeqTree::Pattern(pattern_match)
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                SeqTree::Not(pattern.resolve_pattern(src_dict, trie, resolved_src)?)
            }
            _ => return Err(anyhow!("Unexpected expression type for resolving sequence: {:?}", self)),
        })
    }

    pub fn resolve_rule(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<&'src str, Match>,
        resolved_seq: &HashMap<&'src str, SeqTree>,
    ) -> Result<Rule> {
        Ok(match self {
            Self::Rule(conditions) => {
                if conditions.is_empty() {
                    return Err(anyhow!("Rule must have at least one condition"));
                }
                let resolved_conditions = conditions.iter().map(|(condition, span)|
                        condition.resolve_rule(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)
                    ).collect::<Result<_>>()?;
                Rule::Rule(resolved_conditions)
            }
            Self::Pattern(_) | Self::Any(_) => {
                Rule::Pattern(self.resolve_pattern(src_dict, trie, resolved_src)?)
            }
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                Rule::Repeat(repetition.clone(), Box::new(pattern.resolve_rule(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?))
            }
            Self::Seq(_) => {
                Rule::Seq(self.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?)
            }
            Self::Ident(ident) => {
                let seq = resolved_seq.get(ident);
                let pattern = resolved_pattern.get(ident);
                match (seq, pattern) {
                    (Some(seq), None)     => Rule::Seq(seq.clone()),
                    (None, Some(pattern)) => Rule::Pattern(pattern.clone()),
                    (Some(_), Some(_))    => return Err(anyhow!("Identifier in rule condition cannot refer to both a sequence and a pattern: {}", ident)),
                    (None, None)          => return Err(anyhow!("Unknown identifier in rule condition: {}", ident)),
                }
            }
            Self::Not(boxed) => {
                let (condition, span) = boxed.as_ref();
                let pos_match =
                match condition {
                    Self::Any(exprs) => Match::Any(exprs.iter().map(|(e, span)| match e
                        {
                            Self::Pattern(_) => e.resolve_pattern(src_dict, trie, resolved_src),
                            Self::Ident(ident) => {
                                resolved_pattern.get(*ident)
                                    .cloned()
                                    .ok_or_else(|| anyhow!("Unknown identifier in condition Any: {}", ident))
                            }
                            _ => Err(anyhow!("Unexpected expression type in condition Any: {:?}", e)),
                        }).collect::<Result<_>>()?),
                    Self::Ident(ident) => {
                        let pattern = resolved_pattern.get(*ident);
                        match pattern {
                            Some(pattern) => pattern.clone(),
                            None => return Err(anyhow!("Unknown identifier in Not condition: {}", ident)),
                        }
                    }
                    _ => condition.resolve_pattern(src_dict, trie, resolved_src)?,
                };
                Rule::Pattern(Match::No(Box::new(pos_match)).optimise())
            }
            _ => return Err(anyhow!("Unexpected expression type for resolving rule: {:?}", self)),
        })
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
#[derive(Debug, Clone)]
pub enum SeqTree {
    Pattern(Match),
    Not(Match),
    Repeat(Repetition, Box<Self>),
    // TODO: Enable permutations with splatting (Julia nomenclature)
    Perm(Vec<Self>),
    Seq(Vec<Self>),
}

#[derive(Debug)]
pub enum Rule {
    Pattern(Match),
    Repeat(Repetition, Box<Self>),
    Seq(SeqTree),
    Rule(Vec<Rule>),
}

// -------------------------------------------------
// Processing AST into Semantics Model
// -------------------------------------------------
pub fn resolve_ast(
      declarations: &Vec<Declaration>,
      src_dict: &HashMap<SrcName, DomainSrcId>,
      trie: &Trie,
) -> Result<HashMap<String, Rule>>
{
    let mut resolved_src_dict = HashMap::new();
    for decl in declarations.iter() {
        let (expr, span) = &decl.body;
        match &decl.decl_type {
            DeclType::SrcId => {
                let resolved_src_id = expr.resolve_src(&src_dict).expect("Failed to resolve source identifier");
                debug!("Resolved {:?} into: {:?}", expr, resolved_src_id);
                resolved_src_dict.insert(decl.name, resolved_src_id);
            },
            _ => (),
        }
    }

    let mut resolved_pattern_dict = HashMap::new();
    for decl in declarations.iter() {
        let (expr, span) = &decl.body;
        match &decl.decl_type {
            DeclType::Pattern => {
                let resolved_pattern = expr.resolve_pattern(&src_dict, &trie, &resolved_src_dict).context("Failed to resolve pattern")?;
                debug!("Resolved {:?} into: {:?}", expr, resolved_pattern);
                resolved_pattern_dict.insert(decl.name, resolved_pattern);
            },
            _ => (),
        }
    }

    let mut resolved_seq_dict = HashMap::new();
    for decl in declarations.iter() {
        let (expr, span) = &decl.body;
        match &decl.decl_type {
            DeclType::Sequence => {
                let resolved_seq = expr.resolve_seq(&src_dict, &trie, &resolved_src_dict, &resolved_pattern_dict).context("Failed to resolve sequence")?;
                debug!("Resolved {:?} into: {:?}", expr, resolved_seq);
                resolved_seq_dict.insert(decl.name, resolved_seq);
            },
            _ => (),
        }
    }

    let mut resolved_rule_dict = HashMap::new();
    for decl in declarations.iter() {
        let (expr, span) = &decl.body;
        match &decl.decl_type {
            DeclType::Rule => {
                let resolved_rule = expr.resolve_rule(&src_dict, &trie, &resolved_src_dict, &resolved_pattern_dict, &resolved_seq_dict).context("Failed to resolve rule")?;
                debug!("Resolved {:?} into: {:?}", expr, resolved_rule);
                resolved_rule_dict.insert(decl.name.to_owned(), resolved_rule);
            },
            _ => (),
        }
    }

    Ok(resolved_rule_dict)
}
