//! AST - Abstract Syntax Tree types for the filter DSL
//!
//! This module defines the types that represent parsed filter scripts.
//! These types form the intermediate representation before conversion
//! to the semantic model.

use std::collections::HashMap;

use chumsky::prelude::*;

use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};

use crate::error::Error;

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

/// Represents an event source identifier.
///
/// Source IDs identify where events originate from in the simulation.
/// They can be either resolved (numeric encoding) or unresolved (named).
///
/// Variants:
/// - `None`                    - No source specified
/// - `Mat(u16)`                - Material source with numeric ID
/// - `Surf(u16)`               - Surface source with numeric ID
/// - `MatSurf(u16)`            - Material-Surface source with numeric ID
/// - `Light(u16)`              - Light source with numeric ID
/// - `Detector(u16)`           - Detector source with numeric ID
/// - `MatName(&'src str)`      - Material source by name (to be resolved)
/// - `SurfName(&'src str)`     - Surface source by name
/// - `MatSurfName(&'src str)`  - Material-Surface source by name
/// - `LightName(&'src str)`    - Light source by name
/// - `DetectorName(&'src str)` - Detector source by name
#[derive(Debug, Clone)]
pub enum SrcId<'src> {
    None,
    // Resolved
    Mat(u16),
    Surf(u16),
    MatSurf(u16),
    Light(u16),
    Detector(u16),
    // To look up
    MatName(&'src str),
    SurfName(&'src str),
    MatSurfName(&'src str),
    LightName(&'src str),
    DetectorName(&'src str),
}

macro_rules! get_src_id {
    ($subt:ident, $name:expr, $dict:expr) => {
        match $dict.get(&SrcName::$subt($name)) {
            Some(src_id) => Ok(src_id.clone()),
            None => Err(Error::Unspanned(format!(
                "Unknown source name: {}(\"{}\")",
                stringify!($subt),
                $name
            ))),
        }
    };
}

impl<'a> SrcId<'a> {
    pub fn parse_id(src_id_type: &str, id: u16) -> Result<Self, Error> {
        match src_id_type {
            "Mat"              => Ok(Self::Mat(id)),
            "Surf"             => Ok(Self::Surf(id)),
            "MatSurf"          => Ok(Self::MatSurf(id)),
            "Light"            => Ok(Self::Light(id)),
            "Detector" | "Det" => Ok(Self::Detector(id)),
            _ => Err(Error::Unspanned(format!("Unknown source id type: {}", src_id_type))),
        }
    }
    pub fn parse_name(src_id_type: &str, name: &'a str) -> Result<Self, Error> {
        match src_id_type {
            "Mat"              => Ok(Self::MatName(name)),
            "Surf"             => Ok(Self::SurfName(name)),
            "MatSurf"          => Ok(Self::MatSurfName(name)),
            "Light"            => Ok(Self::LightName(name)),
            "Detector" | "Det" => Ok(Self::DetectorName(name)),
            _ => Err(Error::Unspanned(format!("Unknown source id type: {}", src_id_type))),
        }
    }
    pub fn resolve(&self, dict: &HashMap<SrcName, DomainSrcId>) -> Result<DomainSrcId, Error> {
        Ok(match self {
            Self::None            => DomainSrcId::None,
            Self::Mat(n)          => DomainSrcId::Mat(*n),
            Self::Surf(n)         => DomainSrcId::Surf(*n),
            Self::MatSurf(n)      => DomainSrcId::MatSurf(*n),
            Self::Light(n)        => DomainSrcId::Light(*n),
            Self::Detector(n)     => DomainSrcId::Detector(*n),
            Self::MatName(n)      => get_src_id!(Mat, n.to_string(), dict)?,
            Self::SurfName(n)     => get_src_id!(Surf, n.to_string(), dict)?,
            Self::MatSurfName(n)  => get_src_id!(MatSurf, n.to_string(), dict)?,
            Self::LightName(n)    => get_src_id!(Light, n.to_string(), dict)?,
            Self::DetectorName(n) => get_src_id!(Detector, n.to_string(), dict)?,
        })
    }
}

/// Specifies repetition count for pattern matching.
///
/// Defines how many times a pattern must occur for a match.
/// Corresponds to quantifier syntax in the filter DSL.
///
/// | Variant         | DSL Syntax  | Meaning         |
/// |-----------------|-------------|-----------------|
/// | `Unit`          | (none)      | Exactly once    |
/// | `Optional`      | `?`         | 0 or 1 times    |
/// | `OneOrMore`     | `+`         | 1 or more times |
/// | `ZeroOrMore`    | `*`         | 0 or more times |
/// | `NTimes(n)`     | `{n}`       | Exactly n times |
/// | `AtLeast(n)`    | `{n,}`      | n or more times |
/// | `AtMost(n)`     | `{,n}`      | 0 to n times    |
/// | `Interval(n,m)` | `{n,m}`     | n to m times    |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Repetition {
    Unit,                   // '' Pass-through, no repetition = {1,1}
    Optional,               // '?' = {0,1}
    OneOrMore,              // '+' = {1,}
    ZeroOrMore,             // '*' = {0,}
    NTimes(usize),          // '{n}' = {n,n}
    AtLeast(usize),         //'{n,}': + = {1,}, * = {0,}
    AtMost(usize),          // '{,m}' = {0,m}
    Interval(usize, usize), // '{n,m}': ? = {0,1}
}

impl Repetition {
    pub fn min(&self) -> usize {
        match self {
            Self::Unit => 1,
            Self::Optional => 0,
            Self::OneOrMore => 1,
            Self::ZeroOrMore => 0,
            Self::NTimes(n) => *n,
            Self::AtLeast(n) => *n,
            Self::AtMost(_) => 0,
            Self::Interval(n, _) => *n,
        }
    }
    pub fn max(&self) -> Option<usize> {
        match self {
            Self::Unit => Some(1),
            Self::Optional => Some(1),
            Self::OneOrMore => None,
            Self::ZeroOrMore => None,
            Self::NTimes(n) => Some(*n),
            Self::AtLeast(_) => None,
            Self::AtMost(m) => Some(*m),
            Self::Interval(_, m) => Some(*m),
        }
    }
    pub fn check(&self, count: usize) -> bool {
        let min = self.min();
        let max = self.max();
        count >= min && max.map_or(true, |max| count <= max)
    }
}

#[derive(Debug, Clone)]
/// Represents an expression in the filter DSL.
///
/// Expressions are the core building blocks of patterns and rules.
/// They define what events to match and how to combine matching criteria.
///
/// Variants:
/// - `X`                                      - Don't care / wildcard (matches anything)
/// - `Ident(&'src str)`                       - User-defined identifier
/// - `Field(&'src str)`                       - Event field (e.g., Material, Elastic)
/// - `LedgerPath(&'src str)`                  - Path to ledger file
/// - `SignalsPath(&'src str)`                 - Path to signals file
/// - `Any(Vec<Spanned<Self>>)`                - Match any of several patterns
/// - `Not(Box<Spanned<Self>>)`                - Negation
/// - `Repeat(Repetition, Box<Spanned<Self>>)` - Repetition modifier
/// - `Seq(Vec<Spanned<Self>>)`                - Sequence (ordered)
/// - `Perm(Vec<Spanned<Self>>)`               - Permutation (any order)
/// - `Rule(Vec<Spanned<Self>>)`               - Rule condition
/// - `Pattern(Vec<Spanned<Self>>)`            - Pattern (alternation via `|`)
/// - `SrcId(SrcId<'src>)`                     - Source ID reference
pub enum Expr<'src> {
    X,
    Ident(&'src str),
    Field(&'src str),
    LedgerPath(&'src str),
    SignalsPath(&'src str),
    Any(Vec<Spanned<Self>>),

    Not(Box<Spanned<Self>>),
    Repeat(Repetition, Box<Spanned<Self>>),
    Seq(Vec<Spanned<Self>>),
    Perm(Vec<Spanned<Self>>),
    Rule(Vec<Spanned<Self>>), // e.g. (repetition, pattern), seq, pattern, !pattern
    Pattern(Vec<Spanned<Self>>), // e.g. MCRT | Material | Elastic | X | water_id
    SrcId(SrcId<'src>),
}

#[derive(Debug)]
/// The type of a declaration.
///
/// Indicates what kind of declaration a [`Declaration`] contains.
pub enum DeclType {
    SrcId,
    Pattern,
    Sequence,
    Rule,
    LedgerPath,
    SignalsPath,
}

#[derive(Debug)]
/// A declaration in the filter DSL.
///
/// Declarations are the top-level statements in a filter script,
/// defining sources, patterns, sequences, and rules.
pub struct Declaration<'src> {
    /// The name of the declaration
    pub name: &'src str,
    /// The type of declaration
    pub decl_type: DeclType,
    /// Source span for error reporting
    pub span: Span,
    /// The declaration body (expression)
    pub body: Spanned<Expr<'src>>,
}
