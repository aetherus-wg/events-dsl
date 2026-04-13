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
    // 'X' -Matches any event field or pattern
    DontCare,
    // "any[" ... "]" matches any field/pattern with the specified values
    Any,
    // "perm[" ... "]" matches patterns in any permutation/order
    Perm,
    // - "seq[ ... ]" inline sequence
    Seq,
    // "field <field_ident> = <expr>"
    FieldDecl,
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
    // MCRT, Emission, Detection
    PipelineId(&'src str),
    // Material, Interface, Elastic, etc. => Must define a dictionary of Event names that are
    // reserved
    EventId(&'src str),
    // "match for "Mat", "MatSurf", "Surf", "LightId", "DetectorId": <SrcIdName>("<name>") or
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
            Token::DontCare      => write!(f, "X"       ),
            Token::Any           => write!(f, "any"     ),
            Token::Perm          => write!(f, "perm"    ),
            Token::Seq           => write!(f, "seq"     ),
            Token::SrcDecl       => write!(f, "src"     ),
            Token::FieldDecl     => write!(f, "field"   ),
            Token::PatternDecl   => write!(f, "pattern" ),
            Token::SeqDecl       => write!(f, "sequence"),
            Token::RuleDecl          => write!(f, "rule"    ),
            Token::Ident(s)      => write!(f, "{s}"     ),
            Token::PipelineId(s) => write!(f, "{s}"     ),
            Token::EventId(s)    => write!(f, "{s}"     ),
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
    let dont_care = just('X').to(Token::DontCare);

    // A parser for identifiers and keywords
    let keyword =
               text::ascii::keyword("src"     ).to(Token::SrcDecl)
           .or(text::ascii::keyword("field"   ).to(Token::FieldDecl))
           .or(text::ascii::keyword("pattern" ).to(Token::PatternDecl))
           .or(text::ascii::keyword("sequence").to(Token::SeqDecl))
           .or(text::ascii::keyword("rule"    ).to(Token::RuleDecl))
           .or(text::ascii::keyword("any"     ).to(Token::Any))
           .or(text::ascii::keyword("perm"    ).to(Token::Perm))
           .or(text::ascii::keyword("seq"     ).to(Token::Seq));

    let pipeline_id = text::ascii::keyword("MCRT")
        .or(text::ascii::keyword("Emission"))
        .or(text::ascii::keyword("Detection"))
        .map(Token::PipelineId);

    let src_id = text::ascii::keyword("Mat")
        .or(text::ascii::keyword("MatSurf"))
        .or(text::ascii::keyword("Surf"))
        .or(text::ascii::keyword("LightId"))
        .or(text::ascii::keyword("DetectorId"))
        .map(Token::SrcId);

    let event_id = text::ascii::ident()
        .filter(move |&s| dict.contains(s))
        .map(Token::EventId);

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
        .or(pipeline_id)
        .or(event_id)
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
pub enum SrcId<'src, S>
//where
//    T: aetherus_events::raw::RawField + std::fmt::Debug,
{
    Primitive(S),
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

impl SrcId<'_, aetherus_events::SrcId> {
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
}


#[derive(Debug)]
pub enum Value<'src, T>
{
    DontCare,
    Ident(&'src str),
    Primitive(T),
    // NOTE: There is no reason to support nested "any" since it trivially flattens
    Any(Vec<Self>),
    Not(Box<Self>),
}

// WARN: We don't allow `any[Field]` construction for now or
// use of identifier and/or negation,
// hence only DontCare (X) and Field name can be used
type Field<'src> = Value<'src, &'src str>;

#[derive(Debug)]
pub struct Pattern<'src, S>
{
    pipeline: &'src str,
    event: Vec<Field<'src>>,
    src: Value<'src, SrcId<'src, S>>,
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
    Primitive((Repetition, Value<'src, Pattern<'src>>)),
    // TODO: Enable permutations with splatting (Julia nomenclature)
    Perm(Vec<Self>),
    // NOTE: Nested sequence are unrolled
    Seq(Vec<Self>),
}

#[derive(Debug)]
pub enum Condition<'src> {
    Pattern(Repetition, Pattern<'src>),
    Sequence(SeqTree<'src>),
}

#[derive(Debug)]
pub struct Rule<'src> {
    name: &'src str,
    conditions: Vec<Value<'src, Condition<'src>>>,
}

#[derive(Debug)]
pub enum SyntaxTree<'src> {
    Declarations(Vec<(Self, Span)>),
    SrcDecl(&'src str, Vec<Spanned<Self>>),
    PatternDecl(&'src str, Box<Spanned<Self>>),
    SeqDecl(&'src str, Box<Spanned<Self>>),
    RuleDecl(&'src str, Box<Spanned<Self>>),
    Seq(Spanned<SeqTree<'src>>),
    Src(Spanned<SrcId<'src>>),
    Pattern(Spanned<Pattern<'src>>),
    Rule(Spanned<Rule<'src>>),
}

pub enum DeclarationBody<'src> {
    Src(Spanned<SrcId<'src>>),
    Pattern(Spanned<Pattern<'src>>),
    Seq(Spanned<SeqTree<'src>>),
    Rule(Spanned<Rule<'src>>),
}

pub struct Declaration<'src> {
    //name: &'src str,
    span: Span,
    body: DeclarationBody<'src>,
}

fn expr_parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<SyntaxTree<'src>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{

        let inline_expr = recursive(|inline_expr| {
            let val = select! {
                Token::Num(n) => Expr::Value(Value::Num(n)),
                Token::Str(s) => Expr::Value(Value::Str(s)),
            }
            .labelled("value");

            let ident = select! { Token::Ident(ident) => ident }.labelled("identifier");

            // A list of expressions
            let items = expr
                .clone()
                .separated_by(just(Token::Ctrl(',')))
                .allow_trailing()
                .collect::<Vec<_>>();

            // A let expression
            let let_ = just(Token::Let)
                .ignore_then(ident)
                .then_ignore(just(Token::Op("=")))
                .then(inline_expr)
                .then_ignore(just(Token::Ctrl(';')))
                .then(expr.clone())
                .map(|((name, val), body)| Expr::Let(name, Box::new(val), Box::new(body)));

            let list = items
                .clone()
                .map(Expr::List)
                .delimited_by(just(Token::Ctrl('[')), just(Token::Ctrl(']')));

            // 'Atoms' are expressions that contain no ambiguity
            let atom = val
                .or(ident.map(Expr::Local))
                .or(let_)
                .or(list)
                // In Nano Rust, `print` is just a keyword, just like Python 2, for simplicity
                .or(just(Token::Print)
                    .ignore_then(
                        expr.clone()
                            .delimited_by(just(Token::Ctrl('(')), just(Token::Ctrl(')'))),
                    )
                    .map(|expr| Expr::Print(Box::new(expr))))
                .map_with(|expr, e| (expr, e.span()))
                // Atoms can also just be normal expressions, but surrounded with parentheses
                .or(expr
                    .clone()
                    .delimited_by(just(Token::Ctrl('(')), just(Token::Ctrl(')'))))
                // Attempt to recover anything that looks like a parenthesised expression but contains errors
                .recover_with(via_parser(nested_delimiters(
                    Token::Ctrl('('),
                    Token::Ctrl(')'),
                    [
                        (Token::Ctrl('['), Token::Ctrl(']')),
                        (Token::Ctrl('{'), Token::Ctrl('}')),
                    ],
                    |span| (Expr::Error, span),
                )))
                // Attempt to recover anything that looks like a list but contains errors
                .recover_with(via_parser(nested_delimiters(
                    Token::Ctrl('['),
                    Token::Ctrl(']'),
                    [
                        (Token::Ctrl('('), Token::Ctrl(')')),
                        (Token::Ctrl('{'), Token::Ctrl('}')),
                    ],
                    |span| (Expr::Error, span),
                )))
                .boxed();

            // Function calls have very high precedence so we prioritise them
            let call = atom.foldl_with(
                items
                    .delimited_by(just(Token::Ctrl('(')), just(Token::Ctrl(')')))
                    .map_with(|args, e| (args, e.span()))
                    .repeated(),
                |f, args, e| (Expr::Call(Box::new(f), args), e.span()),
            );

            // Product ops (multiply and divide) have equal precedence
            let op = just(Token::Op("*"))
                .to(BinaryOp::Mul)
                .or(just(Token::Op("/")).to(BinaryOp::Div));
            let product = call
                .clone()
                .foldl_with(op.then(call).repeated(), |a, (op, b), e| {
                    (Expr::Binary(Box::new(a), op, Box::new(b)), e.span())
                });

            // Sum ops (add and subtract) have equal precedence
            let op = just(Token::Op("+"))
                .to(BinaryOp::Add)
                .or(just(Token::Op("-")).to(BinaryOp::Sub));
            let sum = product
                .clone()
                .foldl_with(op.then(product).repeated(), |a, (op, b), e| {
                    (Expr::Binary(Box::new(a), op, Box::new(b)), e.span())
                });

            // Comparison ops (equal, not-equal) have equal precedence
            let op = just(Token::Op("=="))
                .to(BinaryOp::Eq)
                .or(just(Token::Op("!=")).to(BinaryOp::NotEq));
            let compare = sum
                .clone()
                .foldl_with(op.then(sum).repeated(), |a, (op, b), e| {
                    (Expr::Binary(Box::new(a), op, Box::new(b)), e.span())
                });

            compare.labelled("expression").as_context()
        });


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
        "Material".to_string(),
        "Interface".to_string(),
        "Reflector".to_string(),
        "Elastic".to_string(),
        "Inelastic".to_string(),
        "Henyey-Greenstein".to_string(),
        "HG".to_string(),
        "Rayleigh".to_string(),
        "Mie".to_string(),
        "Raman".to_string(),
        "Fluorescence".to_string(),
        "Forward".to_string(),
        "Backward".to_string(),
        "Side".to_string(),
        "Any".to_string(),
    ]);

    let tokens = lexer(dict)
        .parse(src)
        .into_result()
        .unwrap_or_else(|errs| parse_failure(&errs[0], src));

    println!("Tokens: {}", tokens.iter().map(|t| t.0.to_string()).join(" "));

}
