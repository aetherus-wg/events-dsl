# Predicates

The predicates are identical to a parser generator or BNF description, since our
sequence matching has very much in common to the logic behind a parser
combinator. The predicates are inspired from
[pest.rs](https://docs.rs/pest/latest/pest/) with the change that the operator
shows as a prefix in our grammar.

## Negation: `!`

The `!` unnary/monadic operator signifies that the pattern described should not
be matched, not progress the event chains and move on to check next pattern.

## Repetition

| Repetition syntax | Meaning                          |
|-------------------|----------------------------------|
| `? <pattern>`     | Optionally match pattern         |
| `\* <pattern>`    | Match pattern zero or more times |
| `+ <pattern>`     | Match pattern one or more times  |
| `{n} <pattern>`   | Match pattern exactly `n` times  |
| `{m,n} <pattern>` | Match pattern between `m` and `n` times |
| `{,n} <pattern>`  | Match pattern at most `n` times  |
| `{n,} <pattern>`  | Match pattern at least `n` times |
