use std::{fmt::Display, str::FromStr};

use anyhow::{Error, anyhow};

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
