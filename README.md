# Eldritch-Trace filtering DSL

This project defines the grammar of our DSL, implements the parser and
translation to the AST to be used by the filtering methods in `aetherus-events`.

## Specification

## Predicates

Sequence, patterns and fields can have unary predicates.
- "!": Don't match
- "?": Optional, can appear once or none
- "*": Match for any number of times
- "+": Match for any number of times that find at least one match
- "{n,m}": Match for at least n times and at most m times
- "{,m}": Match for at most m times
- "{n,}": Match for at least n times
- "{n}: Match exactly n times

### Field

The field can only have the "!" operator, to check bits mask non equality.
Otherwise, the normal bits match is used.

### Pattern

Patterns can have any of the unary predicates listed above

### Sequence

Sequences can only have an "{n}" predicate that will be unrolled and flatten the
sequence.

> ![NOTE]
> More advanced features to check for non match with "!" could be added later for
> more complex checks on the sequence, but otherwise keep it simple.

## List constructs

- `any`: Allow match to any of the members listed
  - `src`
  - `field`: Not allowed for now
  - `pattern`
- `perm`: Allow match in any order
  - `pattern`
- `seq`: Allow match in the order specified
  - `pattern`
  - `seq`
  - `perm`
  - `any[pattern]`

## Resources

- [chumsky](https://docs.rs/chumsky/latest/chumsky/index.html)
- [pest](pest.rs)
- [Bachus-Naur Form](https://en.wikipedia.org/wiki/Backus%E2%80%93Naur_form)

## TODO

- [X] Vim syntax highlight
- [ ] Tree-sitter parser and syntax highlight
  - [tree-sitter docs](https://tree-sitter.github.io/tree-sitter/)
  - [contributing
  language](https://github.com/nvim-treesitter/nvim-treesitter/blob/main/CONTRIBUTING.md#Parsers)

## Filter DSL - Grammar visualisation

We are looking into how we could visualise the Parser tree, since it can be
quite daunting to look at the parser combinator code and figure out what is
happening.

This requires first to have the grammar specified in BNF/EBNF, then parse and
produce **railroad** diagram.

### Diagram generators

- [AMAZING online converter and viewer on bottlecaps](https://www.bottlecaps.de/ebnf-convert/)
- [pest_railroad: `pest` grammar to SVG](https://github.com/nu11ptr/pest_railroad)
- [bnf2railroad: (E)BNF grammar to SVG/HTML](https://github.com/willy610/bnf2railroad)
- [ebnsf: BNF to SVG using `railroad-JS`](https://crates.io/crates/ebnsf)
- [railroad-diagrams: JS/Python lib to gen
SVG/Unicode](https://github.com/tabatkins/railroad-diagrams/tree/gh-pages)
- [ebnf2ps: EBNF to EPS/FIG](https://github.com/FranklinChen/Ebnf2ps)
  - [PDF
  Documentation](https://raw.githubusercontent.com/FranklinChen/Ebnf2ps/master/doc/doc.pdf)
- [bottlecaps ebnf-convert](https://www.bottlecaps.de/ebnf-convert/)
