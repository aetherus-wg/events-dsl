use chumsky::{input::ValueInput, prelude::*};

use crate::tokenizer::Token;

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

#[derive(Debug, Clone)]
pub enum SrcId<'src>
//where
//    T: aetherus_events::raw::RawField + std::fmt::Debug,
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
pub struct Declaration<'src> {
    name: &'src str,
    span: Span,
    body: Spanned<Expr<'src>>,
}

pub fn expr_parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Declaration<'src>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let src_id_name = select!{Token::SrcId(ty) => ty}
        .then(
            select!{Token::Str(ident) => ident}
                .delimited_by(just(Token::Ctrl('(')), just(Token::Ctrl(')')))
        )
        .map_with(|(ty, ident), e|
            (Expr::SrcId(SrcId::parse_name(ty, ident)), e.span())
        );
    let src_id_val = select!{Token::SrcId(ty) => ty}
        .then(
            select!{Token::Num(val) => val}
                .delimited_by(just(Token::Ctrl('(')), just(Token::Ctrl(')')))
        )
        .map_with(|(ty, id), e|
            (Expr::SrcId(SrcId::parse_id(ty, id)), e.span())
        );
    let src_id = src_id_name.or(src_id_val);

    let src_id_items =
        src_id.clone()
        .separated_by(just(Token::Ctrl(',')))
        .collect::<Vec<_>>();

    //let x = just(Token::X).map_with(|_, e| (Expr::X, e.span()));

    let src_id_any =
            just(Token::Any)
            .ignore_then(src_id_items.delimited_by(just(Token::Ctrl('[')), just(Token::Ctrl(']'))))
            .map_with(|src_id_items, e| (Expr::Any(src_id_items), e.span()));

    let src_id_value =
        src_id
        .or(src_id_any)
        .or(select!{Token::Ident(ident) => Expr::Ident(ident)}.map_with(|val, e| (val, e.span())))
        .or(just(Token::X).map_with(|_, e| (Expr::X, e.span())));

    let fields = select! {
            Token::FieldId(f) => Expr::Field(f),
            //Token::Ident(s)   => Expr::Ident(s),
            Token::X          => Expr::X,
        }
        .map_with(|field_expr, e| (field_expr, e.span()))
        .separated_by(just(Token::Concat))
        .at_least(1)
        .collect::<Vec<_>>();

    let pattern = fields
        .then_ignore(just(Token::Concat))
        .then(src_id_value.clone())
        .map_with(|(fields, src_id), e| {
            let mut all_fields = fields;
            all_fields.push(src_id);
            (Expr::Pattern(all_fields), e.span())
        });

    let inline_pattern = pattern.clone()
        .or(
            select!{Token::Ident(ident) => ident}
                .map_with(|ident, e| (Expr::Ident(ident), e.span()))
        );

    let predicated_pattern = just(Token::Predicates('!'))
        .ignore_then(inline_pattern.clone())
        .map_with(|p, e| (Expr::Not(Box::new(p)), e.span()))
        .or(inline_pattern.clone());

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
        .then(
            predicated_pattern.clone()
            .or(
                select!{Token::X => Expr::X}
                    .map_with(|expr, e| (expr, e.span()))
            )
        )
        .map_with(|(r, p), e| (Expr::Repeat(r, Box::new(p)), e.span()))
        .or(predicated_pattern.map_with(|p, e| (Expr::Repeat(Repetition::Unit, Box::new(p)), e.span())));

    let pattern_items = repetition_pattern.clone()
        .separated_by(just(Token::Ctrl(',')))
        .allow_trailing()
        .collect::<Vec<_>>();

    let seq = just(Token::Seq)
            .ignore_then(just(Token::Ctrl('[')))
            .ignore_then(pattern_items)
            .then_ignore(just(Token::Ctrl(']')))
            .map_with(|items, e| (Expr::Seq(items), e.span()));


    let decl_src = just(Token::SrcDecl)
        .ignore_then(select!{Token::Ident(ident) => ident})
        .then_ignore(just(Token::Ctrl('=')))
        .then(src_id_value)
        .map_with(|(name, src_id_val), e| Declaration {
            name,
            span: e.span(),
            body: src_id_val,
        })
        .boxed();

    let decl_pattern = just(Token::PatternDecl)
        .ignore_then(select!{Token::Ident(ident) => ident})
        .then_ignore(just(Token::Ctrl('=')))
        .then(pattern)
        .map_with(|(name, pattern), e| Declaration {
            name,
            span: e.span(),
            body: pattern,
        })
        .boxed();

    let decl_seq = just(Token::SeqDecl)
        .ignore_then(select!{Token::Ident(ident) => ident})
        .then_ignore(just(Token::Ctrl('=')))
        .then(seq)
        .map_with(|(name, seq), e| Declaration {
            name,
            span: e.span(),
            body: seq,
        })
        .boxed();

    let decl = decl_src
        .or(decl_seq)
        .or(decl_pattern);

    decl.repeated().at_least(1).collect::<Vec<_>>()
}
