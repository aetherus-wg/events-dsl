use std::collections::HashMap;
use anyhow::{anyhow, Result};

use chumsky::prelude::*;

use aetherus_events::{SrcId as DomainSrcId, filter::BitsMatch, ledger::SrcName};

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

/// Events SrcId encoded in 15:0 bits of the encoding
/// The ledger holds the mapping of the SrcId names and their encoded value,
/// hence the need to map to the resolved ledger::SrcId
#[derive(Debug, Clone)]
pub enum SrcId<'src>
{
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
            None => Err(anyhow!("Unknown source name: {}(\"{}\")", stringify!($subt), $name)),
        }
    };
}

impl<'a> SrcId<'a> {
    pub fn parse_id(src_id_type: &str, id: u16) -> Result<Self> {
        match src_id_type {
            "Mat"              => Ok(Self::Mat(id)),
            "Surf"             => Ok(Self::Surf(id)),
            "MatSurf"          => Ok(Self::MatSurf(id)),
            "Light"            => Ok(Self::Light(id)),
            "Detector" | "Det" => Ok(Self::Detector(id)),
            _ => Err(anyhow!("Unknown source id type: {}", src_id_type)),
        }
    }
    pub fn parse_name(src_id_type: &str, name: &'a str) -> Result<Self> {
        match src_id_type {
            "Mat"              => Ok(Self::MatName(name)),
            "Surf"             => Ok(Self::SurfName(name)),
            "MatSurf"          => Ok(Self::MatSurfName(name)),
            "Light"            => Ok(Self::LightName(name)),
            "Detector" | "Det" => Ok(Self::DetectorName(name)),
            _ => Err(anyhow!("Unknown source id type: {}", src_id_type)),
        }
    }
    pub fn resolve(&self, dict: &HashMap<SrcName, DomainSrcId>) -> Result<DomainSrcId> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Repetition {
    Unit,         // '' Pass-through, no repetition = {1,1}
    Optional,     // '?' = {0,1}
    OneOrMore,    // '+' = {1,}
    ZeroOrMore,   // '*' = {0,}
    NTimes(usize),  // '{n}' = {n,n}
    AtLeast(usize), //'{n,}': + = {1,}, * = {0,}
    AtMost(usize),  // '{,m}' = {0,m}
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
pub enum DeclType {
    SrcId,
    Pattern,
    Sequence,
    Rule,
    LedgerPath,
    SignalsPath,
}

#[derive(Debug)]
pub struct Declaration<'src> {
    pub name: &'src str,
    pub decl_type: DeclType,
    pub span: Span,
    pub body: Spanned<Expr<'src>>,
}
