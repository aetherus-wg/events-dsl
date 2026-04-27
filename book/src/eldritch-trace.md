# Eldritch-Trace DSL

Eldritch-Trace is a Domain Specific Language (DSL) and semantic model, which is
used to filter sequences of events whose tag contains a `u32` encoded type 
(primarily used with [aetherus-events](https://github.com/aetherus-wg/aetherus-events))

Eldritch-Trace DSL defines the clearly what the patterns and sequences of
interest should be matched to, then the semantic model runs something akin to a
parses combinator in order to find all sequences of events that satisfy the
conditions.

## API Docs

In addition to this book, you may also wish to read [the API
documentation](TOOD: Add link to docs generated in github pages or crates.io)

## License

Eldritch-Trace DSL is pre-emptively licensed as MIT, but might decide to change
that.

## Sample

Here is an example script used to filter the events for the scene described in the
diagram below.


