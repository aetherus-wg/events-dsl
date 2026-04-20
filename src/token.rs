use std::{collections::HashSet, fmt};

use chumsky::prelude::*;

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'src> {
    // - "[...]" used for "any[...]", "seq[...]" and "perm[...]"
    // - "{n,m}" used for repetition counts in interval [n,m]
    // - "{ conditions* }" lists conditions to be all met
    // - "," Separator for list, patterns and inside repetition
    // - "=" Assignment operator for field, pattern and seq declarations
    Ctrl(char), // '=', '[', ']', ',' , '{', '}'
    Predicates(char), // '*', '+', '?', ! = 0+, 1+, {0,1}, 0
    Concat,     // '|' Concatenation operator
    // 'X'=DontCare -Matches any event field or pattern
    X,
    // "any[" ... "]" matches any field/pattern with the specified values
    Any,
    // "perm[" ... "]" matches patterns in any permutation/order
    Perm,
    // - "seq[ ... ]" inline sequence
    Seq,
    // "src <src_ident> = <expr>"
    SrcDecl,
    // "pattern <pattern_ident> = <expr>"
    PatternDecl,
    // - "sequence <seq_ident> = seq?[]" enumerates patterns in sequence order
    SeqDecl,
    // "rule <rule_ident> = { <expr> }"
    RuleDecl,
    // Identifiers for src ids, fields, patterns and sequences
    Ident(&'src str),
    // MCRT, Emission, Detection,
    // Material, Interface, Elastic, etc. => Must define a dictionary of Field names that are
    // reserved
    FieldId(&'src str),
    // "match for "MatId", "MatSurfId", "SurfId", "LightId", "DetectorId": <SrcIdName>("<name>") or
    // <SrcIdName>(<value>) where value can be hex/dec
    SrcId(&'src str),
    Str(&'src str),
    Num(u16),
    Ledger,
    Photons,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::Ctrl(c)       => write!(f, "{c}"     ),
            Token::Predicates(c) => write!(f, "{c}"     ),
            Token::Concat        => write!(f, "|"       ),
            Token::X             => write!(f, "X"       ),
            Token::Any           => write!(f, "any"     ),
            Token::Perm          => write!(f, "perm"    ),
            Token::Seq           => write!(f, "seq"     ),
            Token::SrcDecl       => write!(f, "src"     ),
            Token::PatternDecl   => write!(f, "pattern" ),
            Token::SeqDecl       => write!(f, "sequence"),
            Token::RuleDecl      => write!(f, "rule"    ),
            Token::Ident(s)      => write!(f, "{s}"     ),
            Token::FieldId(s)    => write!(f, "{s}"     ),
            Token::SrcId(s)      => write!(f, "{s}"     ),
            Token::Num(n)        => write!(f, "{n}"     ),
            Token::Str(s)        => write!(f, "\"{s}\"" ),
            Token::Ledger        => write!(f, "ledger"  ),
            Token::Photons       => write!(f, "photons" ),
        }
    }
}

pub fn lexer<'src>(dict: &HashSet<String>
) -> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, extra::Err<Rich<'src, char, Span>>> {

    // A parser for numbers
    // FIXME: Hex parses not working properly
    let hex_num = just("0x")
        .ignore_then(text::int(16))
        .map(|s: &str| u16::from_str_radix(s, 16).unwrap())
        .map(Token::Num);

    let dec_num = text::int(10)
        .to_slice()
        .from_str()
        .unwrapped()
        .map(Token::Num);

    let num = hex_num.or(dec_num);

    // A parser for strings
    let r#str = just('"')
        .ignore_then(none_of('"').repeated().to_slice())
        .then_ignore(just('"'))
        .map(Token::Str);

    // A parser for operators and  control characters (delimiters, semicolons, etc.)
    let ctrl = one_of("=[]{}(),").map(Token::Ctrl);
    let predicate = one_of("*+?!").map(Token::Predicates);
    let concat = just('|').to(Token::Concat);

    // WARN: 'X' is reserved for "don't care", however the character 'X' should be allowed as part
    // of another string
    let dont_care = just('X').to(Token::X);

    // A parser for identifiers and keywords
    let keyword =
               text::ascii::keyword("src"     ).to(Token::SrcDecl    )
           .or(text::ascii::keyword("pattern" ).to(Token::PatternDecl))
           .or(text::ascii::keyword("sequence").to(Token::SeqDecl    ))
           .or(text::ascii::keyword("rule"    ).to(Token::RuleDecl   ))
           .or(text::ascii::keyword("any"     ).to(Token::Any        ))
           .or(text::ascii::keyword("perm"    ).to(Token::Perm       ))
           .or(text::ascii::keyword("seq"     ).to(Token::Seq        ))
           .or(text::ascii::keyword("ledger"  ).to(Token::Ledger     ))
           .or(text::ascii::keyword("photons" ).to(Token::Photons    ));


    let src_id = text::ascii::keyword("Mat")
        .or(text::ascii::keyword("MatSurf"))
        .or(text::ascii::keyword("Surf"))
        .or(text::ascii::keyword("Light"))
        .or(text::ascii::keyword("Detector"))
        .map(Token::SrcId);

    let field_id = text::ascii::ident()
        .filter(move |&s| dict.contains(s))
        .map(Token::FieldId);

    let ident = text::ascii::ident().map(Token::Ident);

    // A single token can be one of the above
    // WARN: Identifiers may not contain 'X' character
    // as that will be parsed as don't care
    let token = num
        .or(r#str)
        .or(ctrl)
        .or(concat)
        .or(predicate)
        .or(keyword)
        .or(src_id)
        .or(field_id)
        .or(dont_care)
        .or(ident);

    let comment = just("#")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    token
        .map_with(|tok, e| (tok, e.span()))
        .padded_by(comment.repeated())
        .padded()
        // If we encounter an error, skip and attempt to lex the next character as a token instead
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}
