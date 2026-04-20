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
    pub fn parse_id(src_id_type: &str, id: u16) -> Self {
        match src_id_type {
            "Mat"              => Self::Mat(id),
            "Surf"             => Self::Surf(id),
            "MatSurf"          => Self::MatSurf(id),
            "Light"            => Self::Light(id),
            "Detector" | "Det" => Self::Detector(id),
            _ => panic!("Unknown source id type: {}", src_id_type),
        }
    }
    pub fn parse_name(src_id_type: &str, name: &'a str) -> Self {
        match src_id_type {
            "Mat"              => Self::MatName(name),
            "Surf"             => Self::SurfName(name),
            "MatSurf"          => Self::MatSurfName(name),
            "Light"            => Self::LightName(name),
            "Detector" | "Det" => Self::DetectorName(name),
            _ => panic!("Unknown source id type: {}", src_id_type),
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

#[derive(Debug, Clone)]
pub enum Repetition {
    Unit,         // '' Pass-through, no repetition = {1,1}
    Optional,     // '?' = {0,1}
    OneOrMore,    // '+' = {1,}
    ZeroOrMore,   // '*' = {0,}
    NTimes(u16),  // '{n}' = {n,n}
    AtLeast(u16), //'{n,}': + = {1,}, * = {0,}
    AtMost(u16),  // '{,m}' = {0,m}
    Interval(u16, u16), // '{n,m}': ? = {0,1}
}

#[derive(Debug, Clone)]
pub enum Expr<'src> {
    X,
    Ident(&'src str),
    Field(&'src str),
    LedgerPath(&'src str),
    PhotonsPath(&'src str),
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
    PhotonsPath,
}

#[derive(Debug)]
pub struct Declaration<'src> {
    pub name: &'src str,
    pub decl_type: DeclType,
    pub span: Span,
    pub body: Spanned<Expr<'src>>,
}
