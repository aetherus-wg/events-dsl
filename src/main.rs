use std::{collections::{HashMap, HashSet}, env, fmt::{self, Debug}, fs};
use ariadne::{sources, Color, Label, Report, ReportKind};
use itertools::Itertools;

use chumsky::{input::ValueInput, prelude::*};
use aetherus_events::{SrcId as DomainSrcId, ledger::SrcName};

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

#[derive(Debug, Clone)]
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
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::Ctrl(c)       => write!(f, "{c}"     ),
            Token::Predicates(c) => write!(f, "{c}"     ),
            Token::Concat        => write!(f, "|"       ),
            Token::X      => write!(f, "X"       ),
            Token::Any           => write!(f, "any"     ),
            Token::Perm          => write!(f, "perm"    ),
            Token::Seq           => write!(f, "seq"     ),
            Token::SrcDecl       => write!(f, "src"     ),
            Token::PatternDecl   => write!(f, "pattern" ),
            Token::SeqDecl       => write!(f, "sequence"),
            Token::RuleDecl          => write!(f, "rule"    ),
            Token::Ident(s)      => write!(f, "{s}"     ),
            Token::FieldId(s)    => write!(f, "{s}"     ),
            Token::SrcId(s)      => write!(f, "{s}"     ),
            Token::Num(n)        => write!(f, "{n}"     ),
            Token::Str(s)        => write!(f, "{s}"     ),
        }
    }
}

fn lexer<'src>(dict: HashSet<String>
) -> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, extra::Err<Rich<'src, char, Span>>> {
    // A parser for numbers
    let num = text::int(10)
        .or(just("0x").ignore_then(text::int(16)))
        .to_slice()
        .from_str()
        .unwrapped()
        .map(Token::Num);

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
               text::ascii::keyword("src"     ).to(Token::SrcDecl)
           .or(text::ascii::keyword("pattern" ).to(Token::PatternDecl))
           .or(text::ascii::keyword("sequence").to(Token::SeqDecl))
           .or(text::ascii::keyword("rule"    ).to(Token::RuleDecl))
           .or(text::ascii::keyword("any"     ).to(Token::Any))
           .or(text::ascii::keyword("perm"    ).to(Token::Perm))
           .or(text::ascii::keyword("seq"     ).to(Token::Seq));

    let src_id = text::ascii::keyword("MatId")
        .or(text::ascii::keyword("MatSurfId"))
        .or(text::ascii::keyword("SurfId"))
        .or(text::ascii::keyword("LightId"))
        .or(text::ascii::keyword("DetectorId"))
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

// -------------------------------------------------
// Parser Tokens -> AST
// -------------------------------------------------

#[derive(Debug)]
pub enum SrcId<'src>
//where
//    T: aetherus_events::raw::RawField + std::fmt::Debug,
{
    Primitive(aetherus_events::SrcId),
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
        $dict.get(&SrcName::$subt($name))
            .unwrap_or_else(|| panic!("Unknown source name: {}", $name))
            .clone()
    };
}

impl<'a> SrcId<'a> {
    pub fn resolve(&self, dict: &HashMap<aetherus_events::ledger::SrcName, aetherus_events::SrcId>) -> Self {
        Self::Primitive(match self {
            Self::Primitive(s)    => *s,
            Self::None            => DomainSrcId::None,
            Self::Mat(n)          => DomainSrcId::Mat(*n),
            Self::Surf(n)         => DomainSrcId::Surf(*n),
            Self::MatSurf(n)      => DomainSrcId::MatSurf(*n),
            Self::Light(n)        => DomainSrcId::Light(*n),
            Self::Detector(n)     => DomainSrcId::Detector(*n),
            Self::MatName(n)      => get_src_id!(Mat, n.to_string(), dict),
            Self::SurfName(n)     => get_src_id!(Surf, n.to_string(), dict),
            Self::MatSurfName(n)  => get_src_id!(MatSurf, n.to_string(), dict),
            Self::LightName(n)    => get_src_id!(Light, n.to_string(), dict),
            Self::DetectorName(n) => get_src_id!(Detector, n.to_string(), dict),
        })
    }
    pub fn parse_id(src_id_type: &str, id: u16) -> Self {
        match src_id_type {
            "MatId"                => Self::Mat(id),
            "SurfId"               => Self::Surf(id),
            "MatSurfId"            => Self::MatSurf(id),
            "LightId"               => Self::Light(id),
            "DetectorId" | "DetId" => Self::Detector(id),
            _ => panic!("Unknown source id type: {}", src_id_type),
        }
    }
    pub fn parse_name(src_id_type: &str, name: &'a str) -> Self {
        match src_id_type {
            "MatId"                => Self::MatName(name),
            "SurfId"               => Self::SurfName(name),
            "MatSurfId"            => Self::MatSurfName(name),
            "LightId"              => Self::LightName(name),
            "DetectorId" | "DetId" => Self::DetectorName(name),
            _ => panic!("Unknown source id type: {}", src_id_type),
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
    Any(Vec<Spanned<Self>>),
    Not(Box<Spanned<Self>>),
}

// WARN: We don't allow `any[Field]` construction for now or
// use of identifier and/or negation,
// hence only DontCare (X) and Field name can be used
type Field<'src> = Value<'src, &'src str>;

#[derive(Debug)]
pub struct Pattern<'src>
{
    fields: Vec<Spanned<Field<'src>>>,
    src: Spanned<Value<'src, SrcId<'src>>>,
}

#[derive(Debug)]
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

// Sequence Tree
#[derive(Debug)]
pub enum SeqTree<'src> {
    Ident(&'src str),
    Primitive((Repetition, Spanned<Value<'src, Pattern<'src>>>)),
    // TODO: Enable permutations with splatting (Julia nomenclature)
    Perm(Vec<Spanned<Self>>),
    // NOTE: Nested sequence are unrolled
    Seq(Vec<Spanned<Self>>),
}

#[derive(Debug)]
pub enum Condition<'src> {
    Pattern(Repetition, Spanned<Value<'src, Pattern<'src>>>),
    Sequence(Spanned<SeqTree<'src>>),
}

#[derive(Debug)]
pub struct Rule<'src> {
    name: &'src str,
    conditions: Vec<Value<'src, Condition<'src>>>,
}

pub enum DeclarationBody<'src> {
    Src(Value<'src, SrcId<'src>>),
    Pattern(Pattern<'src>),
    Seq(SeqTree<'src>),
    Rule(Rule<'src>),
}

pub struct Declaration<'src> {
    name: &'src str,
    span: Span,
    body: Spanned<DeclarationBody<'src>>,
}

fn expr_parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<Vec<Declaration<'src>>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let src_id_name = select!{Token::SrcId(src_id_type) => src_id_type }
        .then_ignore(just(Token::Ctrl('(')))
        .then(select!{Token::Str(src_id_name) => src_id_name})
        .map_with(|(src_id_type, src_id_name), e|
            (SrcId::parse_name(src_id_type, src_id_name), e.span())
        );
    let src_id_val = select!{Token::SrcId(src_id_type) => src_id_type }
        .then_ignore(just(Token::Ctrl('(')))
        .then(select!{Token::Num(src_id_val) => src_id_val})
        .map_with(|(src_id_type, src_id_val), e|
            (SrcId::parse_id(src_id_type, src_id_val), e.span())
        );
    let src_id = src_id_name.or(src_id_val);

    let src_id_items =
        src_id
        .separated_by(just(Token::Ctrl(',')))
        .collect::<Vec<_>>();

    let src_id_any =
            just(Token::Any)
            .ignore_then(just(Token::Ctrl('[')))
            .ignore_then(src_id_items)
            .then_ignore(just(Token::Ctrl(']')))
            .map_with(|src_id_items, e| (Value::Any(src_id_items), e.span()))

    let src_id_value =
        src_id.map_with(|src_id, e| (Value::Primitive(src_id), e.span()))
        .or(src_id_any)
        .or(select!{Token::Ident(ident) => Value::Ident(ident)}.map_with(|val, e| (val, e.span())))
        .or(just(Token::X).map_with(|_, e| (Value::X, e.span())));

    let src_decl = just(Token::SrcDecl)
        .ignore_then(select!{Token::Ident(ident) => ident})
        .then_ignore(just(Token::Ctrl('=')))
        .then(src_id_value)
        .map_with(|(name, src_id_val), e| Declaration {
            name,
            span: e.span(),
            body: DeclarationBody::Src(src_id_val),
        });

    let fields = select! {
            Token::FieldId(f) => Value::Primitive(f),
            Token::Ident(s)   => Value::Ident(s),
            Token::X          => Value::X,
        }
        .separated_by(just(Token::Concat).padded())
        .at_least(1)
        .collect::<Vec<_>>();

    let pattern = fields
        .then_ignore(just(Token::Concat))
        .then(src_id_value)
        .map(|(fields, src_id)| Pattern {
            fields,
            src: src_id,
        });

    let pattern_decl = just(Token::PatternDecl)
        .ignore_then(select!{Token::Ident(ident) => ident})
        .then_ignore(just(Token::Ctrl('=')))
        .then(pattern)
        .map_with(|(name, pattern), e| Declaration {
            name,
            span: e.span(),
            body: DeclarationBody::Pattern((pattern, e.span())),
        });

    let predicated_pattern = just(Token::Predicates('!'))
        .ignore_then(pattern)
        .map(|p| Value::Not(Box::new(Value::Primitive(p))))
        .or(pattern);

    let repetition = select! {
            Token::Predicates('*') => Repetition::ZeroOrMore,
            Token::Predicates('+') => Repetition::OneOrMore,
            Token::Predicates('?') => Repetition::Optional,
        }
        .or(
            just(Token::Ctrl('{'))
                .ignore_then(select! { Token::Num(n) => n }.or_not())
                .then(
                    just(Token::Ctrl(','))
                        .ignore_then(select! { Token::Num(m) => m }.or_not())
                        .or_not()
                )
                .then_ignore(just(Token::Ctrl('}')))
                .map(|(n_opt, m_opt)| match (n_opt, m_opt) {
                    (Some(n), None)          => Repetition::NTimes(n),
                    (Some(n), Some(None))    => Repetition::AtLeast(n),
                    (None, Some(Some(m)))    => Repetition::AtMost(m),
                    (Some(n), Some(Some(m))) => Repetition::Interval(n, m),
                    _                        => Repetition::Unit, // fallback for invalid syntax
                })
        );

    let repetition_pattern = repetition
        .then(predicated_pattern)
        .map(|(r, p)| (r, p))
        .or(predicated_pattern.map(|p| (Repetition::Unit, p)));

    let pattern_items = repetition_pattern
        .separated_by(just(Token::Ctrl(',')))
        .collect::<Vec<_>>();

    let seq_decl = just(Token::SeqDecl)
        .ignore_then(just(Token::Ident))
        .then_ignore(just(Token::Ctrl('=')))
        .then_ignore(just(Token::Seq).or_not())
        .then(pattern_items.delimited_by(just(Token::Ctrl('[')), just(Token::Ctrl(']'))))
        .map_with(|((name, (patterns, pat_span)), e)| Declaration {
            name,
            span: e.span(),
            body: DeclarationBody::Seq((
                SeqTree::Seq(patterns.iter().map(|p| SeqTree::Primitive(p)).collect()),
                pat_span,
            )),
        });

    let decl = src_decl
        .or(pattern_decl)
        .or(seq_decl);

    decl
}


/// -------------------------------------------------
/// Semantic Model
/// -------------------------------------------------

//pub enum Field<'src> {
//    Wildcard,
//    Ident(&'src str),
//    PipelineId(u8),
//    PipelineName(&'src str),
//    SubEvent(&'src str), // named event/sub-event
//    Event(Vec<Field<'src>>), // named events
//    SrcId(u16),
//    SrcName(&'src str),
//}
//
//pub enum FieldValue {
//    Pipeline(BitsMatch<u8>),
//    Event(BitsMatch<u8>),
//    Src(BitsMatch<u16>),
//}
//
//pub enum Expr<'src> {
//    Field(Field<'src>),
//    // (pipeline, event, src_id)
//    //  - event can be Concat(..) = SuperEvent | SubEvent | SubSubEvent
//    Pattern((Field<'src>, Box<Expr<'src>>, Field<'src>)),
//    Any(Vec<Expr<'src>>),
//    Perm(Vec<Expr<'src>>),
//    Seq(Vec<Expr<'src>>),
//    Qualifier(Vec<Expr<'src>>),
//    Concat(Box<Expr<'src>>, Box<Expr<'src>>),
//}

/// -------------------------------------------------
/// Control Domain Model from Semantics Model
/// -------------------------------------------------

//pub struct BitsMatch {
//    mask: u32,
//    value: u32,
//}
//
//pub enum PatternMatch {
//    Positive(BitsMatch),
//    Negative(BitsMatch),
//    Composite{pos: BitsMatch, neg: BitsMatch},
//}

// -----------------------------------------------
// Parsing helpers
// -----------------------------------------------

fn failure(
    msg: String,
    label: (String, SimpleSpan),
    extra_labels: impl IntoIterator<Item = (String, SimpleSpan)>,
    src: &str,
) -> ! {
    let fname = "example";
    Report::build(ReportKind::Error, (fname, label.1.into_range()))
        .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
        .with_message(&msg)
        .with_label(
            Label::new((fname, label.1.into_range()))
                .with_message(label.0)
                .with_color(Color::Red),
        )
        .with_labels(extra_labels.into_iter().map(|label2| {
            Label::new((fname, label2.1.into_range()))
                .with_message(label2.0)
                .with_color(Color::Yellow)
        }))
        .finish()
        .print(sources([(fname, src)]))
        .unwrap();
    std::process::exit(1)
}

fn parse_failure(err: &Rich<impl fmt::Display>, src: &str) -> ! {
    failure(
        err.reason().to_string(),
        (
            err.found()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "end of input".to_string()),
            *err.span(),
        ),
        err.contexts()
            .map(|(l, s)| (format!("while parsing this {l}"), *s)),
        src,
    )
}

fn main() {
    let filename = env::args().nth(1).expect("Expected file argument");
    let src = &fs::read_to_string(&filename).expect("Failed to read file");

    let dict = HashSet::from([
        "MCRT".to_string(),
        "Emission".to_string(),
        "Detection".to_string(),
        "Material".to_string(),
        "Interface".to_string(),
        "Reflector".to_string(),
        "Elastic".to_string(),
        "Inelastic".to_string(),
        "HenyeyGreenstein".to_string(),
        "Rayleigh".to_string(),
        "Mie".to_string(),
        "Raman".to_string(),
        "Fluorescence".to_string(),
        "Forward".to_string(),
        "Backward".to_string(),
        "Side".to_string(),
        "Unknown".to_string(),
    ]);

    let tokens = lexer(dict)
        .parse(src)
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], src));

    println!("Tokens: {}", tokens.iter().map(|t| t.0.to_string()).join(" "));

}
