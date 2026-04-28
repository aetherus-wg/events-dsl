use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};
use et_dsl::Check;
use et_dsl::ast::{Repetition, SrcId};
use et_dsl::model::{Match, Predicate, Rule, RuleCond, Seq, SeqTree};
use std::collections::HashMap;

fn make_test_src_dict() -> HashMap<SrcName, DomainSrcId> {
    let mut dict = HashMap::new();
    dict.insert(SrcName::Mat("seawater".to_string()), DomainSrcId::Mat(5));
    dict.insert(SrcName::Mat("glass".to_string()), DomainSrcId::Mat(3));
    dict.insert(SrcName::Mat("air".to_string()), DomainSrcId::Mat(0));
    dict.insert(
        SrcName::Surf("TargetToy".to_string()),
        DomainSrcId::Surf(12),
    );
    dict.insert(
        SrcName::Surf("TargetTube".to_string()),
        DomainSrcId::Surf(15),
    );
    dict.insert(
        SrcName::MatSurf("Water:Water_material".to_string()),
        DomainSrcId::MatSurf(7),
    );
    dict.insert(SrcName::Light("laser".to_string()), DomainSrcId::Light(0));
    dict
}

mod match_evaluation {
    use super::*;
    use et_encoding::bits::BitsMatch;

    #[test]
    fn test_match_x_always_passes() {
        let m = Match::X;
        assert!(m.check(0));
        assert!(m.check(u32::MAX));
        assert!(m.check(0xDEADBEEF));
    }

    #[test]
    fn test_match_bits_exact_value() {
        let bm = BitsMatch {
            mask: 0xFFFF,
            value: 0x1234,
        };
        let m = Match::Bits(bm);

        assert!(m.check(0x00001234));
        assert!(m.check(0xFFFF1234));
        assert!(!m.check(0x00001235));
        assert!(!m.check(0x00001230));
    }

    #[test]
    fn test_match_not() {
        let bm = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let m = Match::Not(Box::new(Match::Bits(bm)));

        assert!(!m.check(0x00000042));
        assert!(!m.check(0xFFFFFF42));
        assert!(m.check(0x00000000));
        assert!(m.check(0x00000001));
    }

    #[test]
    fn test_match_and_both_required() {
        let bm1 = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let bm2 = BitsMatch {
            mask: 0xFF00,
            value: 0xAA00,
        };
        let m = Match::And(Box::new(Match::Bits(bm1)), Box::new(Match::Bits(bm2)));

        assert!(m.check(0x0000AA42));
        assert!(!m.check(0x00004200));
        assert!(!m.check(0x0000AA00));
        assert!(!m.check(0x00000042));
    }

    #[test]
    fn test_match_any_passes() {
        let bm1 = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let bm2 = BitsMatch {
            mask: 0xFF,
            value: 0xAA,
        };
        let m = Match::Any(vec![Match::Bits(bm1), Match::Bits(bm2)]);

        assert!(m.check(0x00000042));
        assert!(m.check(0x000000AA));
        assert!(!m.check(0x00000000));
    }

    #[test]
    fn test_match_optimise_to_x() {
        let bm = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let m = Match::Any(vec![Match::X, Match::Bits(bm)]);
        let optimised = m.optimise();

        assert!(matches!(optimised, Match::X));
    }

    #[test]
    fn test_match_optimise_removes_redundant_x() {
        let bm = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let m = Match::And(Box::new(Match::X), Box::new(Match::Bits(bm.clone())));
        let optimised = m.optimise();

        assert_eq!(optimised, Match::Bits(bm));
    }

    #[test]
    fn test_match_optimise_flattens_nested_any() {
        let inner = Match::Any(vec![
            BitsMatch {
                mask: 0xFF,
                value: 0x42,
            }
            .into(),
            BitsMatch {
                mask: 0xFF,
                value: 0xAA,
            }
            .into(),
        ]);
        let m = Match::Any(vec![
            inner,
            BitsMatch {
                mask: 0xFF00,
                value: 0xBB00,
            }
            .into(),
        ]);
        let optimised = m.optimise();

        assert!(matches!(optimised, Match::Any(_)));
        if let Match::Any(inner_vec) = optimised {
            assert_eq!(inner_vec.len(), 3);
        } else {
            panic!("Expected Any after optimisation");
        }
    }
}

mod seqtree_tests {
    use super::*;
    use et_encoding::bits::BitsMatch;

    #[test]
    fn test_seqtree_optimise_flatten_nested_seq() {
        let bm1 = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let bm2 = BitsMatch {
            mask: 0xFF00,
            value: 0xAA00,
        };
        let tree = SeqTree::Seq(vec![
            SeqTree::Pattern(Predicate::Unit, Match::Bits(bm1)),
            SeqTree::Seq(vec![SeqTree::Pattern(Predicate::Unit, Match::Bits(bm2.clone()))]),
        ]);

        let optimised = tree.optimise();

        if let SeqTree::Seq(items) = optimised {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[1], SeqTree::Pattern(Predicate::Unit, Match::Bits(_))));
        } else {
            panic!("Expected Seq after optimisation");
        }
    }

    #[test]
    fn test_seqtree_optimise_single_element_becomes_pattern() {
        let bm = BitsMatch {
            mask: 0xFF,
            value: 0x42,
        };
        let tree = SeqTree::Seq(vec![SeqTree::Pattern(Predicate::Unit, Match::Bits(bm))]);

        let optimised = tree.optimise();

        assert!(matches!(optimised, SeqTree::Pattern(_, _)));
    }
}

mod src_id_resolution {
    use super::*;

    #[test]
    fn test_resolve_mat_id() {
        let src_id = SrcId::Mat(5);
        let dict = make_test_src_dict();

        let resolved = src_id.resolve(&dict).unwrap();
        assert_eq!(resolved, DomainSrcId::Mat(5));
    }

    #[test]
    fn test_resolve_mat_name() {
        let src_id = SrcId::MatName("seawater");
        let dict = make_test_src_dict();

        let resolved = src_id.resolve(&dict).unwrap();
        assert_eq!(resolved, DomainSrcId::Mat(5));
    }
}

mod encoding_src_id_conversion {
    use super::*;

    #[test]
    fn test_src_id_to_encoding_id() {
        assert!(matches!(
            SrcId::Mat(5).to_encoding_src_id(),
            et_encoding::SrcId::MatId
        ));
        assert!(matches!(
            SrcId::MatName("test").to_encoding_src_id(),
            et_encoding::SrcId::MatId
        ));
        assert!(matches!(
            SrcId::Surf(3).to_encoding_src_id(),
            et_encoding::SrcId::SurfId
        ));
        assert!(matches!(
            SrcId::SurfName("test").to_encoding_src_id(),
            et_encoding::SrcId::SurfId
        ));
        assert!(matches!(
            SrcId::MatSurf(1).to_encoding_src_id(),
            et_encoding::SrcId::MatSurfId
        ));
        assert!(matches!(
            SrcId::MatSurfName("test").to_encoding_src_id(),
            et_encoding::SrcId::MatSurfId
        ));
        assert!(matches!(
            SrcId::Light(0).to_encoding_src_id(),
            et_encoding::SrcId::LightId
        ));
        assert!(matches!(
            SrcId::LightName("laser").to_encoding_src_id(),
            et_encoding::SrcId::LightId
        ));
        assert!(matches!(
            SrcId::Detector(0).to_encoding_src_id(),
            et_encoding::SrcId::DetectorId
        ));
        assert!(matches!(
            SrcId::DetectorName("sensor").to_encoding_src_id(),
            et_encoding::SrcId::DetectorId
        ));
    }
}
