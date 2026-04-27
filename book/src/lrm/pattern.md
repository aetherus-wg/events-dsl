# Pattern Declaration

Patterns are described as concatenation of field specifiers, that are described
in the encoding specification used, don't care (X) fields when they are required
to be masked out and a source field that can be explicitly constructed, or used
from a previous `src` declaration.

```
pattern water_scatter = MCRT | Material | Elastic | X | Mat("water")
pattern water_scatter = MCRT | Material | Elastic | X | any[Mat(0), Mat("water")]
pattern water_scatter = MCRT | Material | Elastic | X | water_src
```
