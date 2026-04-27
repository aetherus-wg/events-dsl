use std::{collections::HashMap};
use rand::RngExt;

use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};
use eldritch_dsl::ast::{Repetition, SrcId};

#[test]
fn test_repetition_unit() {
    let rep = Repetition::Unit;
    assert_eq!(rep.min(), 1);
    assert_eq!(rep.max(), Some(1));
    assert!(!rep.check(0));
    assert!(rep.check(1));
    assert!(!rep.check(2));
}

#[test]
fn test_repetition_optional() {
    let rep = Repetition::Optional;
    assert_eq!(rep.min(), 0);
    assert_eq!(rep.max(), Some(1));
    assert!(rep.check(0));
    assert!(rep.check(1));
    assert!(!rep.check(2));
}

#[test]
fn test_repetition_one_or_more() {
    let rep = Repetition::OneOrMore;
    assert_eq!(rep.min(), 1);
    assert_eq!(rep.max(), None);
    assert!(!rep.check(0));
    assert!(rep.check(1));
    assert!(rep.check(5));
}

#[test]
fn test_repetition_zero_or_more() {
    let rep = Repetition::ZeroOrMore;
    assert_eq!(rep.min(), 0);
    assert_eq!(rep.max(), None);
    assert!(rep.check(0));
    assert!(rep.check(1));
    assert!(rep.check(100));
}

#[test]
fn test_repetition_n_times() {
    let n = (rand::rng().random::<u32>() % 100 + 1) as usize; // Random n between 1 and 100
    let rep = Repetition::NTimes(n);
    assert_eq!(rep.min(), n);
    assert_eq!(rep.max(), Some(n));
    assert!(!rep.check(0));
    assert!(!rep.check(n-1));
    assert!(rep.check(n));
    assert!(!rep.check(n+1));
}

#[test]
fn test_repetition_at_least() {
    let n = (rand::rng().random::<u32>() % 100 + 1) as usize; // Random n between 1 and 100
    let rep = Repetition::AtLeast(n);
    assert_eq!(rep.min(), n);
    assert_eq!(rep.max(), None);
    assert!(!rep.check(0));
    assert!(!rep.check(n-1));
    assert!(rep.check(n));
    assert!(rep.check(n+100));
}

#[test]
fn test_repetition_at_most() {
    let n = (rand::rng().random::<u32>() % 100 + 5) as usize; // Random n between 5 and 104
    let rep = Repetition::AtMost(n);
    assert_eq!(rep.min(), 0);
    assert_eq!(rep.max(), Some(n));
    assert!(rep.check(0));
    assert!(rep.check(3));
    assert!(rep.check(n));
    assert!(!rep.check(n+1));
}

#[test]
fn test_repetition_interval() {
    let rep = Repetition::Interval(2, 4);
    assert_eq!(rep.min(), 2);
    assert_eq!(rep.max(), Some(4));
    assert!(!rep.check(0));
    assert!(!rep.check(1));
    assert!(rep.check(2));
    assert!(rep.check(3));
    assert!(rep.check(4));
    assert!(!rep.check(5));
}

#[test]
fn test_src_id_parse_id() {
    assert!(matches!(SrcId::parse_id("Mat", 5).unwrap(), SrcId::Mat(5)));
    assert!(matches!(
        SrcId::parse_id("Surf", 10).unwrap(),
        SrcId::Surf(10)
    ));
    assert!(matches!(
        SrcId::parse_id("MatSurf", 3).unwrap(),
        SrcId::MatSurf(3)
    ));
    assert!(matches!(
        SrcId::parse_id("Light", 1).unwrap(),
        SrcId::Light(1)
    ));
    assert!(matches!(
        SrcId::parse_id("Detector", 0).unwrap(),
        SrcId::Detector(0)
    ));
    assert!(matches!(
        SrcId::parse_id("Det", 2).unwrap(),
        SrcId::Detector(2)
    ));
    assert!(SrcId::parse_id("Invalid", 5).is_err());
}

#[test]
fn test_src_id_parse_name() {
    assert!(matches!(
        SrcId::parse_name("Mat", "seawater").unwrap(),
        SrcId::MatName("seawater")
    ));
    assert!(matches!(
        SrcId::parse_name("Surf", "TargetToy").unwrap(),
        SrcId::SurfName("TargetToy")
    ));
    assert!(matches!(
        SrcId::parse_name("MatSurf", "Water:Water_material").unwrap(),
        SrcId::MatSurfName("Water:Water_material")
    ));
    assert!(matches!(
        SrcId::parse_name("Light", "laser").unwrap(),
        SrcId::LightName("laser")
    ));
    assert!(matches!(
        SrcId::parse_name("Detector", "sensor").unwrap(),
        SrcId::DetectorName("sensor")
    ));
    assert!(SrcId::parse_name("Invalid", "name").is_err());
}

#[test]
fn test_src_id_resolve() {
    let mut src_dict = HashMap::new();
    src_dict.insert(SrcName::Mat("seawater".to_string()), DomainSrcId::Mat(5));

    assert_eq!(
        SrcId::Mat(5).resolve(&src_dict).unwrap(),
        DomainSrcId::Mat(5)
    );
    assert_eq!(
        SrcId::MatName("seawater").resolve(&src_dict).unwrap(),
        DomainSrcId::Mat(5)
    );
    assert!(SrcId::MatName("unknown").resolve(&src_dict).is_err());
}

#[test]
fn test_src_id_to_encoding_src_id() {
    assert!(matches!(
        SrcId::Mat(5).to_encoding_src_id(),
        encoding_spec::SrcId::MatId
    ));
    assert!(matches!(
        SrcId::MatName("seawater").to_encoding_src_id(),
        encoding_spec::SrcId::MatId
    ));
    assert!(matches!(
        SrcId::Surf(3).to_encoding_src_id(),
        encoding_spec::SrcId::SurfId
    ));
    assert!(matches!(
        SrcId::MatSurf(1).to_encoding_src_id(),
        encoding_spec::SrcId::MatSurfId
    ));
    assert!(matches!(
        SrcId::Light(0).to_encoding_src_id(),
        encoding_spec::SrcId::LightId
    ));
    assert!(matches!(
        SrcId::Detector(2).to_encoding_src_id(),
        encoding_spec::SrcId::DetectorId
    ));
    assert!(matches!(
        SrcId::None.to_encoding_src_id(),
        encoding_spec::SrcId::SrcId
    ));
}
