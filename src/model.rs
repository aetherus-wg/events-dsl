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
            Self::X => SeqTree::Pattern(Predicate::Unit, Match::X),
            Self::Seq(exprs) => SeqTree::Seq(
                exprs
                    .iter()
                    .map(|(e, span)| e.resolve_seq(src_dict, trie, resolved_src, resolved_pattern))
                    .collect::<Result<_>>()?
            ),
            Self::Perm(_exprs) => todo!("Permutation feature not implemented yet"),
            //Self::Perm(exprs) => SeqTree::Perm(exprs.iter().map(|(e, span)| e.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)).collect()?),
            Self::Any(exprs) => SeqTree::Pattern(
                Predicate::Unit,
                Match::Any(
                    exprs.iter()
                         .map(|(e, span)| e.resolve_pattern(src_dict, trie, resolved_src))
                         .collect::<Result<_>>()?
                ).optimise()
            ),
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?;
                if let SeqTree::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
                    SeqTree::Pattern(Predicate::Repeat(repetition.clone()), pattern_match)
                } else {
                    return Err(anyhow!("Expected a pattern in sequence repetition, found: {:?}", pattern));
                }
            },
            Self::Ident(pattern_name) => {
                match resolved_pattern.get(*pattern_name) {
                    Some(pattern_match) => SeqTree::Pattern(Predicate::Unit, pattern_match.clone()),
                    None => return Err(anyhow!("Unknown pattern in sequence: {}", pattern_name)),
                }
            }
            Self::Pattern(_expr) => {
                let pattern_match = self.resolve_pattern(src_dict, trie, resolved_src)?;
                SeqTree::Pattern(Predicate::Unit, pattern_match)
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern.resolve_seq(src_dict, trie, resolved_src, resolved_pattern)?;
                if let SeqTree::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
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
                RuleCond::Pattern(Predicate::Unit, self.resolve_pattern(src_dict, trie, resolved_src)?)
            }
            Self::Repeat(repetition, boxed) => {
                let (pattern, span) = boxed.as_ref();
                // TODO: Also support Ident
                let pred_pattern_match = pattern.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?;
                if let RuleCond::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
                    RuleCond::Pattern(Predicate::Repeat(repetition.clone()), pattern_match)
                } else {
                    return Err(anyhow!("Expected a pattern in repetition, found: {:?}", pattern));
                }
            }
            Self::Any(exprs) => RuleCond::Pattern(
                Predicate::Unit,
                Match::Any(exprs.iter().map(|(expr, span)|
                    {
                        let inner_rule_cond = expr.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?;
                        if let RuleCond::Pattern(Predicate::Unit, pattern_match) = inner_rule_cond {
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
                    (None, Some(pattern)) => RuleCond::Pattern(Predicate::Unit, pattern.clone()),
                    (Some(_), Some(_))    => return Err(anyhow!("Identifier in rule condition cannot refer to both a sequence and a pattern: {}", ident)),
                    (None, None)          => return Err(anyhow!("Unknown identifier in rule condition: {}", ident)),
                }
            }
            Self::Not(boxed) => {
                let (pattern, span) = boxed.as_ref();
                let pred_pattern_match = pattern.resolve_rule_cond(src_dict, trie, resolved_src, resolved_pattern, resolved_seq)?;
                if let RuleCond::Pattern(Predicate::Unit, pattern_match) = pred_pattern_match {
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
            RuleCond::Pattern(Predicate::Unit, pattern_match)      => pattern_match.check(events_chain),
            RuleCond::Pattern(Predicate::Not, pattern_match)       => !pattern_match.check(events_chain),
            RuleCond::Pattern(Predicate::Repeat(r), pattern_match) => r.check(events_chain.iter().fold(0, |acc, &event| if pattern_match.check(event) { acc + 1} else { acc })),
            RuleCond::Seq(_) => todo!("Sequence evaluation not implemented yet"),
        })
    }
}

pub fn find_forward_uid_rule(ledger: &Ledger, rule: &Rule) -> Vec<Uid> {
    let mut found_uids: Vec<Uid> = Vec::new();

    #[derive(Clone, Debug)]
    pub enum CondTraverse<'a> {
        Pattern{
            pred: Predicate,
            event_match: &'a Match,
            cnt: usize,
        },
        Seq{
            seq_idx: usize,
            seq: &'a Seq,
            cnt: usize,
        }
    }
    #[derive(Clone)]
    pub struct RuleTraverse<'a> {
        pub uid: Uid,
        pub cond_idx: usize,
        pub conds: Vec<CondTraverse<'a>>,
    }

    impl RuleTraverse<'_> {
        pub fn satfisfied(&self) -> bool {
            if self.cond_idx != 0 {
                return false;
            }
            for cond in self.conds.iter() {
                match cond {
                    CondTraverse::Pattern{pred, event_match:_, cnt} => {
                        match pred {
                            Predicate::Unit => return false,
                            Predicate::Not => (),
                            Predicate::Repeat(r) => {
                                if !r.check(*cnt) {
                                    return false;
                                }
                            }
                        }
                    }
                    CondTraverse::Seq{seq_idx, seq, cnt:_} => {
                        if *seq_idx < seq.0.len() {
                            return false;
                        }
                    }
                }
            }
            true
        }
    }

    let mut stack: Vec<RuleTraverse> = Vec::new();

    // Initial stack entries
    let mut conds = Vec::new();
    for cond in rule.0.iter().rev() {
        match cond {
            RuleCond::Pattern(pred, event_match) => conds.push(CondTraverse::Pattern{pred: pred.clone(), event_match, cnt: 0}),
            RuleCond::Seq(seq) => {
                conds.push(CondTraverse::Seq{seq_idx: 0, seq, cnt: 0});
            }
        }
    }
    for &uid in ledger.get_start_events() {
        stack.push(RuleTraverse{ uid, cond_idx: 0, conds: conds.clone()});
    }

    // Ledger traversal loop
    while !stack.is_empty() {
        let mut rule = stack.pop().unwrap();

        //println!("Evaluating rule at UID: {:?}, condition index: {}, conditions: {:?}", rule.uid, rule.cond_idx, rule.conds);

        let mut pass = true;
        let mut remove = false;
        let mut bifurcate = false;

        let mut recheck = true;
        while recheck {
            let cond = &mut rule.conds[rule.cond_idx];
            recheck = false;
            match cond {
                CondTraverse::Pattern{pred, event_match, cnt} => {
                    let event_check = event_match.check(rule.uid.event);
                    match pred {
                        Predicate::Unit => {
                            if event_check {
                                // Condition satified and ready to remove
                                remove = true;
                            }
                        }
                        Predicate::Not => {
                            if event_check {
                                pass = false;
                            }
                        }
                        Predicate::Repeat(r) => {
                            if event_check {
                                *cnt = *cnt + 1;
                                if r.min() > *cnt {
                                    // Just increment count and stay on the same sequence condition until reaching the minimum required repetitions
                                } else {
                                    if let Some(upper) = r.max() {
                                        if *cnt > upper {
                                            pass = false;
                                        }
                                    } else {
                                        remove = true;
                                    }
                                }
                            }
                        }
                    }
                }
                CondTraverse::Seq{seq_idx, seq, cnt} => {
                    if *seq_idx >= seq.0.len() {
                        pass = false;
                        break;
                    }

                    let (pred, pattern_match) = &seq.0[*seq_idx];
                    let event_check = pattern_match.check(rule.uid.event);
                    match pred {
                        // Indexes of sequence conditions that are currently in neg check (predicate=!),
                        // to be evaluated on the same UID without advancing
                        Predicate::Not => {
                            if event_check {
                                pass = false;
                            } else {
                                recheck = true;
                                *cond = CondTraverse::Seq{seq_idx: *seq_idx + 1, cnt: 0, seq};
                            }
                        }
                        Predicate::Unit => {
                            if event_check {
                                *cond = CondTraverse::Seq{seq_idx: *seq_idx + 1, cnt: 0, seq};
                            } else {
                                pass = false;
                            }
                        }
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
                                            *cond = CondTraverse::Seq{seq_idx: *seq_idx + 1, cnt: 0, seq};
                                        } else {
                                             // Satisfied, but we might allow this pattern to
                                            // consume more events
                                            bifurcate = true;
                                        }
                                    } else {
                                        bifurcate = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(!remove || !bifurcate, "Bifurcate(Seq) and Remove(Pattern) are mutually exclusive flags");

        if pass {
            let cond_idx = rule.cond_idx;
            let next_uids = ledger.get_next(&rule.uid);

            if remove {
                rule.conds.remove(rule.cond_idx);
                if rule.conds.len() == cond_idx {
                    rule.cond_idx = 0;
                }
            } else {
                if rule.cond_idx + 1 == rule.conds.len() {
                    rule.cond_idx = 0;
                } else {
                    rule.cond_idx = rule.cond_idx + 1;
                }
            }


            if bifurcate {
                let mut bifurcated_rule = rule.clone();
                if let CondTraverse::Seq{seq_idx, seq, cnt:_} = bifurcated_rule.conds[cond_idx] {
                    bifurcated_rule.conds[cond_idx] = CondTraverse::Seq{seq_idx: seq_idx + 1, cnt: 0, seq};
                } else {
                    panic!("Expected a sequence condition for bifurcation");
                }

                if rule.cond_idx == 0 {
                    for &next_uid in next_uids.iter() {
                        stack.push(RuleTraverse{ uid: next_uid, cond_idx: bifurcated_rule.cond_idx, conds: bifurcated_rule.conds.clone()});
                    }
                    if next_uids.is_empty() && rule.satfisfied() {
                        //println!("Found a match for rule with UID: {:?}", rule.uid);
                        if !found_uids.contains(&rule.uid) {
                            found_uids.push(rule.uid);
                        }
                    }
                } else {
                    stack.push(bifurcated_rule);
                }
            }

            if rule.cond_idx == 0 {
                for &next_uid in next_uids.iter() {
                    stack.push(RuleTraverse{ uid: next_uid, cond_idx: rule.cond_idx, conds: rule.conds.clone()});
                }
                if next_uids.is_empty() && rule.satfisfied() {
                    //println!("Found a match for rule with UID: {:?}", rule.uid);
                    if !found_uids.contains(&rule.uid) {
                        found_uids.push(rule.uid);
                    }
                }
            } else {
                stack.push(rule);
            }
        }
    }

    found_uids
}
