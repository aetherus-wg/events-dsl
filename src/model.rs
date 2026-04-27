use std::collections::HashMap;

use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};
use encoding_spec::{pattern::{self, Pattern}, trie::Trie, bits::BitsMatch};
use log::debug;

use crate::{ast::{DeclType, Declaration, Expr, Repetition, SrcId}, error::Error, failure};
use crate::Check;

// -------------------------------------------------
// AST -> Semantics Model
// -------------------------------------------------

#[derive(Debug, Clone)]
pub enum Match {
    X,
    Bits(BitsMatch),
    Not(Box<Match>),
    And(Box<Match>, Box<Match>),
    Any(Vec<Match>),
}

impl Match {

    pub fn optimise(self) -> Self {
        match self {
            Match::And(ref left, ref right) => match (left.as_ref(), right.as_ref()) {
                (Match::Bits(bits1), Match::Bits(bits2)) => Match::Bits(bits1.combine(&bits2)),
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

impl Check<u32> for Match {
    fn check(&self, event_encoded: u32) -> bool {
        match self {
            Match::X => true,
            Match::Bits(bits_match) => bits_match.check(event_encoded),
            Match::Not(bits_match) => !bits_match.check(event_encoded),
            Match::And(left, right) => left.check(event_encoded) && right.check(event_encoded),
            Match::Any(matches) => matches.iter().any(|m| m.check(event_encoded)),
        }
    }
}

impl Check<&[u32]> for Match
{
    fn check(&self, events_chain: &[u32]) -> bool {
        events_chain.iter().all(|event_encoded| self.check(*event_encoded))
    }
}

impl Into<encoding_spec::SrcId> for SrcId<'_> {
    fn into(self) -> encoding_spec::SrcId {
        match self {
            Self::None            => encoding_spec::SrcId::SrcId,
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

impl SrcId<'_> {
    pub fn to_encoding_src_id(&self) -> encoding_spec::SrcId {
        match self {
            Self::None            => encoding_spec::SrcId::SrcId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    Unit,
    Not,
    Repeat(Repetition),
}

// Sequence Tree
#[derive(Debug, Clone)]
pub enum SeqTree {
    Pattern(Predicate, Match),
    // TODO: Enable permutations with splatting (Julia nomenclature)
    Perm(Vec<Self>),
    // TODO: Flatten nested sequences
    Seq(Vec<Self>),
}

impl SeqTree {
    /// Flatten nested Seq
    pub fn optimise(self) -> Self {
        match self {
            SeqTree::Seq(seq) => {
                let mut flattened = Vec::new();
                for s in seq.into_iter() {
                    match s.optimise() {
                        SeqTree::Seq(inner) => flattened.extend(inner),
                        other => flattened.push(other),
                    }
                }
                if flattened.len() == 1 {
                    flattened[0].to_owned()
                } else {
                    SeqTree::Seq(flattened)
                }
            }
            other => other,
        }
    }

    pub fn lower(self) -> Seq {
        match self {
            SeqTree::Pattern(pred, pattern_match) => Seq(vec![(pred, pattern_match)]),
            SeqTree::Seq(seq) => {
                let mut lowered = Vec::new();
                for s in seq.into_iter() {
                    lowered.extend(s.lower().0);
                }
                Seq(lowered)
            }
            SeqTree::Perm(_seq) => todo!("Permutation feature not implemented yet"),
        }
    }
}

#[derive(Debug)]
pub struct Seq(pub Vec<(Predicate, Match)>);

#[derive(Debug)]
pub enum RuleCond {
    Pattern(Predicate, Match),
    Seq(Seq),
}

#[derive(Debug)]
pub struct Rule(pub Vec<RuleCond>);

// -------------------------------------------------
// Processing AST into Semantics Model
// -------------------------------------------------
pub fn resolve_ast(
    src: &str,
    declarations: &Vec<Declaration>,
    src_dict: &HashMap<SrcName, DomainSrcId>,
    trie: &Trie,
) -> HashMap<String, Rule>
{
    let mut resolved_src_dict = HashMap::new();
    for decl in declarations.iter() {
        let (expr, span) = &decl.body;
        match &decl.decl_type {
            DeclType::SrcId => {
                let resolved_src_id = expr
                    .resolve_src(&src_dict)
                    .unwrap_or_else(|e| {
                        let err = e.with_span(*span);
                        failure(
                            format!("Failed to resolve source identifier {:?}: {}", expr, err.to_string()),
                            ("not found inside Ledger".into(), err.span().unwrap()),
                            None,
                            src,
                        )
                    });

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
                let resolved_pattern = expr
                    .resolve_pattern(&src_dict, &trie, &resolved_src_dict)
                    .unwrap_or_else(|e| {
                        let err = e.with_span(*span);
                        failure(
                            format!("Failed to resolve pattern {:?}: {}", expr, err.to_string()),
                            ("not found inside Trie".into(), err.span().unwrap()),
                            None,
                            src,
                        );
                    });
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
                let resolved_seq = expr
                    .resolve_seq(&src_dict, &trie, &resolved_src_dict, &resolved_pattern_dict)
                    .unwrap_or_else(|e| {
                        let err = e.with_span(*span);
                        failure(
                            format!("Failed to resolve sequence {:?}: {}", expr, err.to_string()),
                            ("failed to resolve sequence".into(), err.span().unwrap()),
                            None,
                            src,
                        );
                    });
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
                let resolved_rule = expr
                    .resolve_rule(&src_dict, &trie, &resolved_src_dict, &resolved_pattern_dict, &resolved_seq_dict)
                    .unwrap_or_else(|e| {
                        let err = e.with_span(*span);
                        failure(
                            format!("Failed to resolve rule {:?}: {}", expr, err.to_string()),
                            ("failed to resolve rule".into(), err.span().unwrap()),
                            None,
                            src,
                        );
                    });
                debug!("Resolved {:?} into: {:?}", expr, resolved_rule);
                resolved_rule_dict.insert(decl.name.to_owned(), resolved_rule);
            },
            _ => (),
        }
    }

    resolved_rule_dict
}

impl<'src> Expr<'src> {
    pub fn resolve_src(&self, src_dict: &HashMap<SrcName, DomainSrcId>) -> Result<(encoding_spec::SrcId, Match), Error> {
        Ok(
        match self {
            Self::X => (encoding_spec::SrcId::SrcId, Match::X),
            Self::SrcId(src_id) => {
                debug!("Resolving source identifier: {:?}", src_id);
                let src_bits_match = src_id
                        .resolve(src_dict)?
                        .bits_match()
                        .into();
                (src_id.clone().into(), Match::Bits(src_bits_match))
            },
            // TODO: If error occur extract the span and propagate it back as Result
            // Perhaps make use of ariadne to properly show diagnostics relevant to code
            Self::Any(exprs) => {
                let src_id_matches: Vec<_> = exprs
                        .iter()
                        .map(|(expr, span)| expr
                            .resolve_src(src_dict)
                            .map_err(|err| err.with_span(*span))
                        )
                        .collect::<Result<_,_>>()?;
                let src_id_type = src_id_matches.iter().fold(encoding_spec::SrcId::SrcId, |acc, (src_id, _)| {
                    acc.combine(&src_id).unwrap()
                });
                let src_id_match = Match::Any(src_id_matches.into_iter().map(|(_, r#match)| r#match).collect());
                debug!("Combined any SrcId types into: ({:?}, {:?})", src_id_type, src_id_match);
                // TODO: flatten nested Any
                (src_id_type, src_id_match.optimise())
            }
            _ => return Err(Error::Unspanned(format!("Unexpected expression type for resolving SrcId: {:?}", self))),
        }
        )
    }

    pub fn resolve_pattern(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>
    ) -> Result<Match, Error> {
        Ok(match self {
            Self::X => Match::X,
            Self::Pattern(fields) => {
                let src_field = match &fields.last() {
                    Some(field) => &field.0,
                    // TODO: Include span information
                    None => return Err(Error::Unspanned("Pattern must have at least one field (the SrcId)".into())),
                };
                let (src_id_type, src_id_match) = match src_field {
                    Expr::SrcId(_) | Expr::Any(_) => {
                        src_field.resolve_src(src_dict)?
                    }
                    Expr::Ident(src_ident) => {
                        match resolved_src.get(*src_ident) {
                            Some(ident) => ident.clone(),
                            // TODO: Include span information
                            None => return Err(Error::Unspanned(format!("Unknown source identifier in pattern: {}", src_ident))),
                        }
                    }
                    _ => return Err(Error::Unspanned(format!("Unexpected expression type for SrcId in pattern: {:?}", src_field))),
                };

                let mut fields: Vec<_> = fields[..fields.len() - 1]
                    .iter()
                    .map(|(field, span)| match field {
                        Expr::X => Ok(pattern::Field::X),
                        Expr::Field(field_id) => Ok(pattern::Field::Field(field_id)),
                        _ => Err(Error::Spanned{msg:format!("Unexpected expression type in pattern fields: {:?}", field), span: *span}),
                    })
                    .collect::<Result<_,_>>()?;

                fields.push(pattern::Field::SrcId(src_id_type));
                let pattern = Pattern(fields);
                let (bits_match, _src_id_type) = trie
                    .get(&pattern)
                    .map_err(|err|
                        Error::Unspanned(format!("{}", err.context("Failed to get pattern from trie")))
                    )?;
                Match::And(Box::new(Match::Bits(bits_match)), Box::new(src_id_match)).optimise()
            }
            Self::Any(exprs) => {
                let bits_match = exprs
                    .iter()
                    .map(|(e, span)| e
                        .resolve_pattern(src_dict, trie, resolved_src)
                        .map_err(|err| err.with_span(*span))
                    ).collect::<Result<_,_>>()?;
                Match::Any(bits_match).optimise()
            }
            _ => return Err(Error::Unspanned(format!("Unexpected expression type for resolving SrcId: {:?}", self))),
        })
    }

    pub fn resolve_seq(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<&'src str, Match>
    ) -> Result<SeqTree, Error> {
        Ok(match self {
            Self::X => SeqTree::Pattern(Predicate::Unit, Match::X),
            Self::Seq(exprs) => SeqTree::Seq(
                exprs
                    .iter()
                    .map(|(e, span)| e
                        .resolve_seq(src_dict, trie, resolved_src, resolved_pattern)
                        .map_err(|err| err.with_span(*span))
                    )
                    .collect::<Result<_,_>>()?
            ),
            Self::Perm(_exprs) => return Err(Error::Unspanned(format!("Permutation feature not implemented yet"))),
            Self::Any(exprs) => SeqTree::Pattern(
                Predicate::Unit,
                Match::Any( exprs
                    .iter()
                    .map(|(e, span)| e
                        .resolve_pattern(src_dict, trie, resolved_src)
                        .map_err(|err| err.with_span(*span))
                    )
                    .collect::<Result<_,_>>()?
                ).optimise()
            ),
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern
                    .resolve_seq(src_dict, trie, resolved_src, resolved_pattern)
                    .map_err(|err| err.with_span(*span))?;
                if let SeqTree::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
                    SeqTree::Pattern(Predicate::Repeat(repetition.clone()), pattern_match)
                } else {
                    return Err(Error::Unspanned(format!("Expected a pattern in sequence repetition, found: {:?}", pattern)).with_span(*span));
                }
            },
            Self::Ident(pattern_name) => {
                match resolved_pattern.get(*pattern_name) {
                    Some(pattern_match) => SeqTree::Pattern(Predicate::Unit, pattern_match.clone()),
                    None => return Err(Error::Unspanned(format!("Unknown pattern in sequence: {}", pattern_name))),
                }
            }
            Self::Pattern(_expr) => {
                let pattern_match = self.resolve_pattern(src_dict, trie, resolved_src)?;
                SeqTree::Pattern(Predicate::Unit, pattern_match)
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern
                    .resolve_seq(src_dict, trie, resolved_src, resolved_pattern)
                    .map_err(|err| err.with_span(*span))?;
                if let SeqTree::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
                    SeqTree::Pattern(Predicate::Not, pattern_match)
                } else {
                    return Err(Error::Unspanned(format!("Expected a pattern in negation, found: {:?}", pattern)).with_span(*span));
                }
            }
            _ => return Err(Error::Unspanned(format!("Unexpected expression type for resolving sequence: {:?}", self))),
        })
    }

    pub fn resolve_rule_cond(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<&'src str, Match>,
        resolved_seq: &HashMap<&'src str, SeqTree>,
    ) -> Result<RuleCond, Error> {
        Ok(match self {
            Self::Pattern(_) => {
                RuleCond::Pattern(Predicate::Unit, self.resolve_pattern(src_dict, trie, resolved_src)?)
            }
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                // TODO: Also support Ident
                let pred_pattern_match = pattern
                    .resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)
                    .map_err(|err| err.with_span(*span))?;
                if let RuleCond::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
                    RuleCond::Pattern(Predicate::Repeat(repetition.clone()), pattern_match)
                } else {
                    return Err(Error::Unspanned(format!("Expected a pattern in repetition, found: {:?}", pattern)).with_span(*span));
                }
            }
            Self::Any(exprs) => RuleCond::Pattern(
                Predicate::Unit,
                Match::Any(exprs.iter().map(|(expr, span)|
                    {
                        let inner_rule_cond = expr
                            .resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)
                            .map_err(|err| err.with_span(*span))?;
                        if let RuleCond::Pattern(Predicate::Unit, pattern_match) = inner_rule_cond {
                            Ok(pattern_match)
                        } else {
                            Err(Error::Unspanned(format!("Unexpected expression type in condition Any: {:?}", expr)).with_span(*span))
                        }
                    }).collect::<Result<_,_>>()?
                ).optimise()
            ),
            Self::Seq(_) => {
                RuleCond::Seq(self
                    .resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?
                    .lower()
                )
            }
            Self::Ident(ident) => {
                let seq = resolved_seq.get(ident);
                let pattern = resolved_pattern.get(ident);
                match (seq, pattern) {
                    (Some(seq), None)     => RuleCond::Seq(seq.clone().lower()),
                    (None, Some(pattern)) => RuleCond::Pattern(Predicate::Unit, pattern.clone()),
                    (Some(_), Some(_))    => return Err(Error::Unspanned(format!("Identifier in rule condition cannot refer to both a sequence and a pattern: {}", ident))),
                    (None, None)          => return Err(Error::Unspanned(format!("Unknown identifier in rule condition: {}", ident))),
                }
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern
                    .resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)
                    .map_err(|err| err.with_span(*span))?;
                if let RuleCond::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
                    RuleCond::Pattern(Predicate::Not, pattern_match.optimise())
                } else {
                    return Err(Error::Unspanned(format!("Expected a pattern in negation, found: {:?}", pattern)).with_span(*span));
                }
            }
            _ => return Err(Error::Unspanned(format!("Unexpected expression type for resolving rule: {:?}", self))),
        })
    }

    pub fn resolve_rule(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<&'src str, Match>,
        resolved_seq: &HashMap<&'src str, SeqTree>,
    ) -> Result<Rule, Error> {
        Ok(match self {
            Self::Rule(conditions) => {
                if conditions.is_empty() {
                    return Err(Error::Unspanned(format!("Rule must have at least one condition")));
                }
                let resolved_conditions = conditions
                    .iter()
                    .map(|(condition, span)|
                        condition
                            .resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)
                            .map_err(|err| err.with_span(*span))
                    )
                    .collect::<Result<_,_>>()?;
                Rule(resolved_conditions)
            }
            _ => return Err(Error::Unspanned(format!("Unexpected expression type for resolving rule: {:?}", self))),
        })
    }
}
