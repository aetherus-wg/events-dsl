# Sequence

A sequence describes the order of event patterns to be matched.

```eldritch-trace
sequence seq_water_backscatter = seq[
    Emission | X | Light(0),
    * X,
    water_scatter,
    * X,
    Detection | X | Detector(0),
]
```
