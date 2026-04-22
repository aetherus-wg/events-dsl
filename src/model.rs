use std::collections::HashMap;

use aetherus_events::{Ledger, SrcId as DomainSrcId, ledger::{SrcName, Uid}};
use anyhow::{Context, Result, anyhow};
use encoding_spec::{pattern::{self, Pattern}, trie::Trie, bits::BitsMatch};
use log::{debug, error};

use crate::ast::{DeclType, Declaration, Expr, Repetition, SrcId};

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

trait Check<T> {
    fn check(&self, value: T) -> bool;
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
    None,
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

impl<'src> Expr<'src> {
    pub fn resolve_src(&self, src_dict: &HashMap<SrcName, DomainSrcId>) -> Result<(encoding_spec::SrcId, Match)> {
        Ok(
        match self {
            Self::X => (encoding_spec::SrcId::SrcId, Match::X),
            Self::SrcId(src_id) => {
                debug!("Resolving source identifier: {:?}", src_id);
                let src_bits_match = src_id.resolve(src_dict)?.bits_match().into();
                (src_id.clone().into(), Match::Bits(src_bits_match))
            },
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
                Match::And(Box::new(Match::Bits(bits_match)), Box::new(src_id_match)).optimise()
            }
            Self::Any(exprs) => {
                let bits_match = exprs.iter().map(|(e, span)| e.resolve_pattern(src_dict, trie, resolved_src)).collect::<Result<_>>()?;
                Match::Any(bits_match).optimise()
            }
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
            Self::X => SeqTree::Pattern(Predicate::None, Match::X),
            Self::Seq(exprs) => SeqTree::Seq(
                exprs
                    .iter()
                    .map(|(e, span)| e.resolve_seq(src_dict, trie, resolved_src, resolved_pattern))
                    .collect::<Result<_>>()?
            ),
            Self::Perm(_exprs) => todo!("Permutation feature not implemented yet"),
            //Self::Perm(exprs) => SeqTree::Perm(exprs.iter().map(|(e, span)| e.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)).collect()?),
            Self::Any(exprs) => SeqTree::Pattern(
                Predicate::None,
                Match::Any(
                    exprs.iter()
                         .map(|(e, span)| e.resolve_pattern(src_dict, trie, resolved_src))
                         .collect::<Result<_>>()?
                ).optimise()
            ),
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?;
                if let SeqTree::Pattern(Predicate::None, pattern_match) = pred_pattern_match {
                    SeqTree::Pattern(Predicate::Repeat(repetition.clone()), pattern_match)
                } else {
                    return Err(anyhow!("Expected a pattern in sequence repetition, found: {:?}", pattern));
                }
            },
            Self::Ident(pattern_name) => {
                match resolved_pattern.get(*pattern_name) {
                    Some(pattern_match) => SeqTree::Pattern(Predicate::None, pattern_match.clone()),
                    None => return Err(anyhow!("Unknown pattern in sequence: {}", pattern_name)),
                }
            }
            Self::Pattern(_expr) => {
                let pattern_match = self.resolve_pattern(src_dict, trie, resolved_src)?;
                SeqTree::Pattern(Predicate::None, pattern_match)
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?;
                if let SeqTree::Pattern(Predicate::None, pattern_match) = pred_pattern_match {
                    SeqTree::Pattern(Predicate::Not, pattern_match)
                } else {
                    return Err(anyhow!("Expected a pattern in negation, found: {:?}", pattern));
                }
            }
            _ => return Err(anyhow!("Unexpected expression type for resolving sequence: {:?}", self)),
        })
    }

    pub fn resolve_rule_cond(
        &self,
        src_dict: &HashMap<SrcName, DomainSrcId>,
        trie: &Trie,
        resolved_src: &HashMap<&'src str, (encoding_spec::SrcId, Match)>,
        resolved_pattern: &HashMap<&'src str, Match>,
        resolved_seq: &HashMap<&'src str, SeqTree>,
    ) -> Result<RuleCond> {
        Ok(match self {
            Self::Pattern(_) => {
                RuleCond::Pattern(Predicate::None, self.resolve_pattern(src_dict, trie, resolved_src)?)
            }
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                // TODO: Also support Ident
                let pred_pattern_match = pattern.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?;
                if let RuleCond::Pattern(Predicate::None, pattern_match) = pred_pattern_match {
                    RuleCond::Pattern(Predicate::Repeat(repetition.clone()), pattern_match)
                } else {
                    return Err(anyhow!("Expected a pattern in repetition, found: {:?}", pattern));
                }
            }
            Self::Any(exprs) => RuleCond::Pattern(
                Predicate::None,
                Match::Any(exprs.iter().map(|(expr, span)|
                    {
                        let inner_rule_cond = expr.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?;
                        if let RuleCond::Pattern(Predicate::None, pattern_match) = inner_rule_cond {
                            Ok(pattern_match)
                        } else {
                            Err(anyhow!("Unexpected expression type in condition Any: {:?}", expr))
                        }
                    }).collect::<Result<_>>()?
                ).optimise()
            ),
            Self::Seq(_) => {
                RuleCond::Seq(self.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?.lower())
            }
            Self::Ident(ident) => {
                let seq = resolved_seq.get(ident);
                let pattern = resolved_pattern.get(ident);
                match (seq, pattern) {
                    (Some(seq), None)     => RuleCond::Seq(seq.clone().lower()),
                    (None, Some(pattern)) => RuleCond::Pattern(Predicate::None, pattern.clone()),
                    (Some(_), Some(_))    => return Err(anyhow!("Identifier in rule condition cannot refer to both a sequence and a pattern: {}", ident)),
                    (None, None)          => return Err(anyhow!("Unknown identifier in rule condition: {}", ident)),
                }
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?;
                if let RuleCond::Pattern(Predicate::None, pattern_match) = pred_pattern_match {
                    RuleCond::Pattern(Predicate::Not, pattern_match.optimise())
                } else {
                    return Err(anyhow!("Expected a pattern in negation, found: {:?}", pattern));
                }
            }
            _ => return Err(anyhow!("Unexpected expression type for resolving rule: {:?}", self)),
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
                        condition.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)
                    ).collect::<Result<_>>()?;
                Rule(resolved_conditions)
            }
            _ => return Err(anyhow!("Unexpected expression type for resolving rule: {:?}", self)),
        })
    }
}

impl Rule {
    pub fn evaluate(&self, events_chain: &[u32]) -> bool {
        self.0.iter().all(|cond| match cond {
            RuleCond::Pattern(Predicate::None, pattern_match)      => pattern_match.check(events_chain),
            RuleCond::Pattern(Predicate::Not, pattern_match)       => !pattern_match.check(events_chain),
            RuleCond::Pattern(Predicate::Repeat(r), pattern_match) => r.check(events_chain.iter().fold(0, |acc, &event| if pattern_match.check(event) { acc + 1} else { acc })),
            RuleCond::Seq(_) => todo!("Sequence evaluation not implemented yet"),
        })
    }
}

pub fn find_forward_uid_rule(ledger: &Ledger, rule: &Rule) -> Vec<Uid> {
    let mut found_uids: Vec<Uid> = Vec::new();

    #[derive(Clone)]
    pub enum CondTraverse<'a> {
        Pattern{
            pred: Predicate,
            event_match: Match,
            cnt: usize,
        },
        Seq{
            seq_idx: usize,
            cnt: usize,
            seq: &'a Seq,
        }
    }
    #[derive(Clone)]
    pub struct RuleTraverse<'a>{
        pub uid: Uid,
        pub conds: Vec<CondTraverse<'a>>,
        pub neg_checks: Vec<usize>,
    }

    let mut stack: Vec<RuleTraverse> = Vec::new();

    // Initial stack entries
    let mut conds = Vec::new();
    let mut neg_checks = Vec::new();
    for cond in rule.0.iter().rev() {
        match cond {
            RuleCond::Pattern(pred, event_match) => conds.push(CondTraverse::Pattern{pred: pred.clone(), event_match: event_match.clone(), cnt: 0}),
            RuleCond::Seq(seq) => {
                // Indicate which neg_checks to first check
                if seq.0[0].0 == Predicate::Not {
                    neg_checks.push(conds.len());
                }
                conds.push(CondTraverse::Seq{seq_idx: 0, cnt: 0, seq});
            }
        }
    }
    for &uid in ledger.get_start_events() {
        stack.push(RuleTraverse{ uid, conds: conds.clone(), neg_checks: neg_checks.clone() });
    }

    // Ledger traversal loop
    while !stack.is_empty() {
        let rule = stack.pop().unwrap();
        let next_uids = ledger.get_next(&rule.uid);

        // The only scenario where we don't advance UID, is when a sequence contains a non match
        // pattern statement.
        if !rule.neg_checks.is_empty() {
            let mut pass = true;
            let mut conds = rule.conds.clone();
            let mut neg_checks = Vec::new();
            for &cond_idx in rule.neg_checks.iter() {
                let (seq_idx, cnt, seq) = match rule.conds[cond_idx] {
                    CondTraverse::Seq{seq_idx, cnt, seq} => (seq_idx, cnt, seq),
                    CondTraverse::Pattern{..} => panic!("Invalid negation check in rule traversal, expected a sequence condition"),
                };

                let (pred, pattern_match) = &seq.0[seq_idx];
                assert_eq!(pred, &Predicate::Not);

                if pattern_match.check(rule.uid.event) {
                    pass = false;
                    break;
                } else {
                    let seq_idx = seq_idx + 1;
                    if seq.0.len() > seq_idx && seq.0[seq_idx].0 == Predicate::Not {
                        neg_checks.push(cond_idx);
                    }
                    conds[cond_idx] = CondTraverse::Seq{seq_idx: seq_idx, cnt: 0, seq};
                }
            }

            if pass {
                // If all negation checks passed, we can safely advance the UID and push the new rule state to the stack
                stack.push(RuleTraverse{ uid: rule.uid, conds, neg_checks});
            }
        }
        // Otherwise, we check conditions and advance or drop UID if not satisfying conditions
        else {
            let mut pass = true;
            // First check pattern conditions, if any of them fail, we drop the UID and don't advance
            let mut conds: Vec<_> = rule.conds
                .into_iter()
                .filter_map(|cond| match cond {
                    CondTraverse::Seq{..} => Some(cond.clone()),
                    CondTraverse::Pattern { ref pred, ref event_match, cnt} => {
                        let event_check = event_match.check(rule.uid.event);
                        match pred {
                            Predicate::None => {
                                if event_check {
                                    None
                                } else {
                                    Some(cond)
                                }
                            },
                            Predicate::Not => {
                                if event_check {
                                    pass = false;
                                    None
                                } else {
                                    Some(cond)
                                }
                            },
                            Predicate::Repeat(r) => {
                                if event_check {
                                    let cnt = cnt + 1;
                                    if let Some(upper) = r.max() {
                                        if cnt > upper {
                                            pass = false;
                                            None
                                        } else {
                                            Some(CondTraverse::Pattern{ pred: pred.clone(), event_match: event_match.clone(), cnt })
                                        }
                                    } else {
                                        Some(cond)
                                    }
                                } else {
                                    Some(cond)
                                }
                            },
                        }
                    }
                })
                .collect();
            if !pass {
                continue;
            }
            let mut neg_checks = Vec::new();
            // WARN: Possible bifuraction in sequence condition for repeated pattern match, when there
            // isn't an upper bound or the upper bound is not reached yet
            for (cond_idx, cond) in conds.iter_mut().enumerate() {
                if let CondTraverse::Seq{seq_idx, cnt, seq} = cond {
                    let (pred, pattern_match) = &seq.0[*seq_idx];
                    let event_check = pattern_match.check(rule.uid.event);
                    match pred {
                        Predicate::None => {
                            if event_check {
                                let seq_idx = *seq_idx + 1;
                                if seq.0.len() > seq_idx && seq.0[seq_idx].0 == Predicate::Not {
                                    neg_checks.push(cond_idx);
                                }
                                *cond = CondTraverse::Seq{seq_idx, cnt: 0, seq};
                            } else {
                                pass = false;
                                break;
                            }
                        },
                        Predicate::Not => panic!("Invalid sequence condition, unexpected negation predicate at seq_idx: {}", seq_idx),
                        Predicate::Repeat(r) => {
                            if event_check {
                                *cnt = *cnt + 1;
                                if r.min() > *cnt {
                                    // Just increment count and stay on the same sequence condition until reaching the minimum required repetitions
                                } else {
                                    if let Some(upper) = r.max() {
                                        assert!(r.min() <= upper, "Invalid repetition predicate with min > max in sequence condition");
                                        if *cnt >= upper {
                                            // Satisfied, move on
                                            let seq_idx = *seq_idx + 1;
                                            if seq.0.len() > seq_idx && seq.0[seq_idx].0 == Predicate::Not {
                                                neg_checks.push(cond_idx);
                                            }
                                            *cond = CondTraverse::Seq{seq_idx, cnt: 0, seq};
                                        } else {
                                             // Satisfied, but we might allow this pattern to
                                            // consume more events
                                            error!("Bifurcation not supported yet. Repeat pattern in seq: {:?}, {:?}", pred, pattern_match);
                                        }
                                    } else {
                                        error!("Bifurcation not supported yet. Repeat pattern in seq: {:?}, {:?}", pred, pattern_match);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if pass {
                for &next_uid in next_uids.iter() {
                    stack.push(RuleTraverse{ uid: next_uid, conds: conds.clone(), neg_checks: neg_checks.clone() });
                }

                if next_uids.is_empty() {
                    let mut pass = true;
                    // Check if conditions have been satisfied
                    for cond in conds.iter() {
                        pass =
                        match cond {
                            CondTraverse::Pattern{pred, event_match: _, cnt} => {
                                match pred {
                                    Predicate::None => false,
                                    Predicate::Not => true,
                                    Predicate::Repeat(r) => r.check(*cnt),
                                }
                            },
                            CondTraverse::Seq{seq_idx, cnt: _, seq} => {
                                if *seq_idx < seq.0.len() {
                                    // Sequence not fully satisfied
                                    false
                                } else {
                                    true
                                }
                            }
                        };
                        if !pass {
                            break;
                        }
                    }

                    if pass {
                        found_uids.push(rule.uid);
                    }
                }
            }
        }
    }

    found_uids
}
