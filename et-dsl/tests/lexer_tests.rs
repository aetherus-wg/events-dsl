use chumsky::Parser;
use et_dsl::token::{Token, lexer};
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
    dict
}

fn full_dict() -> HashSet<String> {
    let mut dict = default_dict();
    dict.insert("X".to_string());
    dict
}

mod lexer_numbers {
    use super::*;

    #[test]
    fn test_decimal_numbers() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("0 42 123 65535").unwrap();
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].0, Token::Num(0)));
        assert!(matches!(tokens[1].0, Token::Num(42)));
        assert!(matches!(tokens[2].0, Token::Num(123)));
        assert!(matches!(tokens[3].0, Token::Num(65535)));
    }

    #[test]
    #[ignore]
    fn test_hex_numbers() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("0x00 0xFF 0x1A 0xABCD").unwrap();
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].0, Token::Num(0)));
        assert!(matches!(tokens[1].0, Token::Num(255)));
        assert!(matches!(tokens[2].0, Token::Num(26)));
        assert!(matches!(tokens[3].0, Token::Num(43981)));
    }
}

mod lexer_strings {
    use super::*;

    #[test]
    fn test_simple_strings() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("\"hello\" \"world\"").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].0, Token::Str("hello")));
        assert!(matches!(tokens[1].0, Token::Str("world")));
    }

    #[test]
    fn test_strings_with_spaces() {
        let dict = default_dict();
        let tokens = lexer(&dict)
            .parse("\"seawater\" \"glass material\"")
            .unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].0, Token::Str("seawater")));
        assert!(matches!(tokens[1].0, Token::Str("glass material")));
    }

    #[test]
    fn test_empty_string() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("\"\"").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::Str("")));
    }
}

mod lexer_control_chars {
    use super::*;

    #[test]
    fn test_all_control_chars() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("=[]{},()").unwrap();
        assert_eq!(tokens.len(), 8);
        assert!(matches!(tokens[0].0, Token::Ctrl('=')));
        assert!(matches!(tokens[1].0, Token::Ctrl('[')));
        assert!(matches!(tokens[2].0, Token::Ctrl(']')));
        assert!(matches!(tokens[3].0, Token::Ctrl('{')));
        assert!(matches!(tokens[4].0, Token::Ctrl('}')));
        assert!(matches!(tokens[5].0, Token::Ctrl(',')));
        assert!(matches!(tokens[6].0, Token::Ctrl('(')));
        assert!(matches!(tokens[7].0, Token::Ctrl(')')));
    }
}

mod lexer_operators {
    use super::*;

    #[test]
    fn test_concat_operator() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("|").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::Concat));
    }

    #[test]
    fn test_all_predicates() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("*+?!").unwrap();
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].0, Token::Predicates('*')));
        assert!(matches!(tokens[1].0, Token::Predicates('+')));
        assert!(matches!(tokens[2].0, Token::Predicates('?')));
        assert!(matches!(tokens[3].0, Token::Predicates('!')));
    }
}

mod lexer_keywords {
    use super::*;

    #[test]
    fn test_all_keywords() {
        let dict = default_dict();
        let tokens = lexer(&dict)
            .parse("src pattern sequence rule any perm seq ledger signals")
            .unwrap();
        assert_eq!(tokens.len(), 9);
        assert!(matches!(tokens[0].0, Token::SrcDecl));
        assert!(matches!(tokens[1].0, Token::PatternDecl));
        assert!(matches!(tokens[2].0, Token::SeqDecl));
        assert!(matches!(tokens[3].0, Token::RuleDecl));
        assert!(matches!(tokens[4].0, Token::Any));
        assert!(matches!(tokens[5].0, Token::Perm));
        assert!(matches!(tokens[6].0, Token::Seq));
        assert!(matches!(tokens[7].0, Token::Ledger));
        assert!(matches!(tokens[8].0, Token::Signals));
    }
}

mod lexer_src_id_types {
    use super::*;

    #[test]
    #[ignore]
    fn test_all_src_id_types() {
        let dict = default_dict();
        let tokens = lexer(&dict)
            .parse("Mat Surf MatSurf Light Detector Det")
            .unwrap();
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0].0, Token::SrcId("Mat")));
        assert!(matches!(tokens[1].0, Token::SrcId("Surf")));
        assert!(matches!(tokens[2].0, Token::SrcId("MatSurf")));
        assert!(matches!(tokens[3].0, Token::SrcId("Light")));
        assert!(matches!(tokens[4].0, Token::SrcId("Detector")));
        assert!(matches!(tokens[5].0, Token::SrcId("Detector")));
    }
}

mod lexer_field_ids {
    use super::*;

    #[test]
    fn test_field_ids_from_dict() {
        let mut dict = full_dict();
        dict.insert("EventType".to_string());

        let tokens = lexer(&dict)
            .parse("MCRT Material Elastic EventType")
            .unwrap();
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].0, Token::FieldId("MCRT")));
        assert!(matches!(tokens[1].0, Token::FieldId("Material")));
        assert!(matches!(tokens[2].0, Token::FieldId("Elastic")));
        assert!(matches!(tokens[3].0, Token::FieldId("EventType")));
    }

    #[test]
    #[ignore]
    fn test_x_dont_care() {
        let dict = full_dict();
        let tokens = lexer(&dict).parse("X").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::X));
    }
}

mod lexer_identifiers {
    use super::*;

    #[test]
    fn test_user_identifiers() {
        let dict = default_dict();
        let tokens = lexer(&dict)
            .parse("water_id glass_interf my_pattern rule_name")
            .unwrap();
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].0, Token::Ident("water_id")));
        assert!(matches!(tokens[1].0, Token::Ident("glass_interf")));
        assert!(matches!(tokens[2].0, Token::Ident("my_pattern")));
        assert!(matches!(tokens[3].0, Token::Ident("rule_name")));
    }
}

mod lexer_comments {
    use super::*;

    #[test]
    fn test_single_line_comment() {
        let dict = default_dict();
        let tokens = lexer(&dict).parse("# This is a comment\nX").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::X));
    }

    #[test]
    #[ignore]
    fn test_comment_at_end_of_line() {
        let dict = full_dict();
        let tokens = lexer(&dict).parse("X # comment").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::X));
    }

    #[test]
    #[ignore]
    fn test_multiple_comments() {
        let dict = full_dict();
        let tokens = lexer(&dict)
            .parse("# comment 1\nX # comment 2\n# comment 3")
            .unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::X));
    }
}

mod lexer_whitespace {
    use super::*;

    #[test]
    #[ignore]
    fn test_spaces() {
        let dict = full_dict();
        let tokens = lexer(&dict).parse("  X   Z  ").unwrap();
        // Just verify tokens exist and have some content
        assert!(tokens.len() >= 1);
    }

    #[test]
    fn test_tabs() {
        let dict = full_dict();
        let tokens = lexer(&dict).parse("\tX\tZ\t").unwrap();
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn test_newlines() {
        let dict = full_dict();
        let tokens = lexer(&dict).parse("X\nY\nZ").unwrap();
        assert_eq!(tokens.len(), 3);
    }
}

mod lexer_complex_patterns {
    use super::*;

    #[test]
    #[ignore]
    fn test_pattern_with_concat() {
        let mut dict = full_dict();
        dict.insert("MCRT".to_string());
        dict.insert("Material".to_string());
        dict.insert("Elastic".to_string());

        let src = "MCRT | Material | Elastic | X | Mat(5)";
        let tokens = lexer(&dict).parse(src).unwrap();

        assert_eq!(tokens.len(), 11);
        assert!(matches!(tokens[0].0, Token::FieldId("MCRT")));
        assert!(matches!(tokens[1].0, Token::Concat));
        assert!(matches!(tokens[2].0, Token::FieldId("Material")));
        assert!(matches!(tokens[3].0, Token::Concat));
        assert!(matches!(tokens[4].0, Token::FieldId("Elastic")));
        assert!(matches!(tokens[5].0, Token::Concat));
        assert!(matches!(tokens[6].0, Token::X));
        assert!(matches!(tokens[7].0, Token::Concat));
        assert!(matches!(tokens[8].0, Token::SrcId("Mat")));
        assert!(matches!(tokens[9].0, Token::Ctrl('(')));
        assert!(matches!(tokens[10].0, Token::Num(5)));
    }

    #[test]
    fn test_any_with_multiple_items() {
        let mut dict = default_dict();
        dict.insert("Mat".to_string());
        dict.insert("MatSurf".to_string());

        let src = "any[Mat(\"seawater\"), MatSurf(\"Water:Water_material\")]";
        let tokens = lexer(&dict).parse(src).unwrap();

        assert!(tokens.len() > 5);
        assert!(matches!(tokens[0].0, Token::Any));
        assert!(matches!(tokens[1].0, Token::Ctrl('[')));
    }
}

mod lexer_token_display {
    use super::*;

    #[test]
    fn test_ctrl_display() {
        assert_eq!(Token::Ctrl('=').to_string(), "=");
        assert_eq!(Token::Ctrl('[').to_string(), "[");
        assert_eq!(Token::Ctrl(']').to_string(), "]");
    }

    #[test]
    fn test_predicates_display() {
        assert_eq!(Token::Predicates('*').to_string(), "*");
        assert_eq!(Token::Predicates('+').to_string(), "+");
        assert_eq!(Token::Predicates('?').to_string(), "?");
        assert_eq!(Token::Predicates('!').to_string(), "!");
    }

    #[test]
    fn test_concat_display() {
        assert_eq!(Token::Concat.to_string(), "|");
    }

    #[test]
    fn test_x_display() {
        assert_eq!(Token::X.to_string(), "X");
    }

    #[test]
    fn test_any_display() {
        assert_eq!(Token::Any.to_string(), "any");
    }

    #[test]
    fn test_ident_display() {
        assert_eq!(Token::Ident("test").to_string(), "test");
    }

    #[test]
    fn test_num_display() {
        assert_eq!(Token::Num(42).to_string(), "42");
        assert_eq!(Token::Num(255).to_string(), "255");
    }

    #[test]
    fn test_str_display() {
        assert_eq!(Token::Str("hello").to_string(), "\"hello\"");
    }

    #[test]
    fn test_field_id_display() {
        assert_eq!(Token::FieldId("MCRT").to_string(), "MCRT");
    }

    #[test]
    fn test_src_id_display() {
        assert_eq!(Token::SrcId("Mat").to_string(), "Mat");
    }

    #[test]
    fn test_ledger_display() {
        assert_eq!(Token::Ledger.to_string(), "ledger");
    }

    #[test]
    fn test_signals_display() {
        assert_eq!(Token::Signals.to_string(), "signals");
    }
}
