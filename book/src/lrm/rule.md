# Rule: `rule`

A `rule` describes the set of conditions to be met by a terminal UID in the
chain of events.

Conditions can be:

- Patterns, predicated or not
- Set of patterns i.e. `any[pattern_1, pattern_2, ...]`
- Sequence to be matched 

Bot patterns and sequences can be written inline the rule declarations or used
as identifier of previously declared values.

```eldritch-trace
rule toy_or_tube_detect = {
    any[toy_surf, tube_surf], # Set match
    ! boundary_surf,          # Predicated indentifier pattern
    seq[
        Emission | X | Light(0),
        * X,
        + water_scatter,      # Repetition of identifier inside sequence
        * X,
        Detection | X | Detector(0),
    ],
    my_other_sequence,
}
```
