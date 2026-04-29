use chumsky::{Parser, input::Input};
use et_dsl::ast::{DeclType, Expr};
use et_dsl::parse::expr_parser;
use et_dsl::lexer::lexer;
use std::collections::HashSet;

fn default_dict() -> HashSet<String> {
    let mut dict = HashSet::new();
    dict.insert("MCRT".to_string());
    dict.insert("Material".to_string());
    dict.insert("Interface".to_string());
    dict.insert("Elastic".to_string());
    dict.insert("Inelastic".to_string());
    dict.insert("Reflector".to_string());
    dict.insert("Emission".to_string());
    dict.insert("Detection".to_string());
    dict.insert("Backward".to_string());
    dict.insert("Forward".to_string());
    dict.insert("Light".to_string());
    dict.insert("Detector".to_string());
    dict.insert("X".to_string());
    dict
}

fn parse_script(src: &str) -> Vec<et_dsl::ast::Declaration<'_>> {
    let dict = default_dict();
    let tokens = lexer(&dict).parse(src).unwrap();
    expr_parser()
        .parse(
            tokens
                .as_slice()
                .map((src.len()..src.len()).into(), |(t, s)| (t, s)),
        )
        .into_result()
        .unwrap()
}

mod ledger_signals {
    use super::*;

    #[test]
    fn test_parse_ledger_path() {
        let src = r#"ledger = "path/to/ledger.json""#;
        let decls = parse_script(src);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "ledger");
        assert!(matches!(decls[0].body.0, Expr::LedgerPath(_)));
        assert!(matches!(decls[0].decl_type, DeclType::LedgerPath));
    }

    #[test]
    fn test_parse_signals_path() {
        let src = r#"signals = "path/to/signals.csv""#;
        let decls = parse_script(src);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "signals");
        assert!(matches!(decls[0].body.0, Expr::SignalsPath(_)));
        assert!(matches!(decls[0].decl_type, DeclType::SignalsPath));
    }
}

mod src_id {
    use super::*;

    #[test]
    fn test_parse_src_id_named() {
        let src = r#"src water_id = Mat("seawater")"#;
        let decls = parse_script(src);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "water_id");
    }

    #[test]
    fn test_parse_src_id_numeric() {
        let src = r#"src air_id = Mat(0)"#;
        let decls = parse_script(src);
        assert_eq!(decls.len(), 1);
    }

    #[test]
    fn test_parse_multiple_src_declarations() {
        let src = r#"
            src water_id = Mat("seawater")
            src glass_id = Mat("glass")
            src air_id = Mat(0)
        "#;
        let decls = parse_script(src);
        assert_eq!(decls.len(), 3);
    }
}
