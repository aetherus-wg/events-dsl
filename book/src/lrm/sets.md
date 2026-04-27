# Sets

- `any`
- `seq`
- `perm`

## Any: `any`

`any` is effectively an OR of a list of src fields or patterns listed:

- `any[Mat(0), Mat("water"), MatSurf("water_interface")]`
- `any[pattern_id_0, MCRT | Material | X | SrcId(0)]`

## Sequence: `seq`

`seq` is the set constructor of a sequence, describing a series of pattern that
must match in order. Each of the patterns might be predicated by a negative or
repetition operator.

Example:
```
seq[
    Emission | X | Light(0),
    * X,
    target_pattern,
    * X,
    Detection | X | Detector(0),
]
```
```
```

## Permutation: `perm`

> ![WARN] This functionality is not covered yet,
> but it should provide the possibility to be consumed only within a sequence,
> such that some events can be described out-of-order.
