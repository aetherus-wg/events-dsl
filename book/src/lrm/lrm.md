# Language Reference Manual

Bachus-Naur Form compatible with ANTLR parser generator:

```ebnf
Declarations = { Declaration };

Declaration = SrcDecl
            | SeqDecl
            | PatternDecl
            | RuleDecl
            ;
Comment     = "#" { !"#" anycharacter } ;

SrcDecl     = "src" Ident "=" SrcIdValue ;
PatternDecl = "pattern" Ident "=" Pattern ;
SeqDecl     = "sequence" Ident "=" Seq ;
RuleDecl    = "rule" Ident "=" "{" ConditionItems "}" ;

SrcIdValue = SrcId | SrcIdAny | Ident | "X" ;

SrcId      = SrcIdName | SrcIdVal ;
SrcIdName  = SrcIdType "(" String ")" ;
SrcIdVal   = SrcIdType "(" Number ")" ;
SrcIdType  = "Mat" | "Surf" | "MatSurf" | "Light" | "Detector" ;

SrcIdAny   = "any" "[" SrcIdItems "]" ;
SrcIdItems = SrcIdItem { "," SrcIdItem } ;
SrcIdItem  = SrcId | Ident ;

Pattern    = Fields "|" SrcIdValue ;
Fields     = FieldExpr { "|" FieldExpr } ;
FieldExpr  = FieldId | "X" ;

InlinePattern       = Pattern | Ident ;
InlinePatternItems  = InlinePattern { "," InlinePattern } ;
PatternAny          = "any" "[" InlinePatternItems "]" ;
PatternSet          = InlinePattern | PatternAny | "X" ;
PredicatedPattern   = "!" PatternSet | PatternSet ;

Repetition = "*"
           | "+"
           | "?"
           | "{" [ Number ] [ "," [ Number ] ] "}"
           ;

RepetitionPattern = Repetition PredicatedPattern
                   | PredicatedPattern ;

PatternItems = RepetitionPattern { "," RepetitionPattern } ;

Seq = "seq" "[" PatternItems "]" ;

Condition      = RepetitionPattern | Seq ;
ConditionItems = Condition { "," Condition } ;

FieldId   = Ident ;
Ident     = letter { letter | digit | "_" | "." } ;
Decimal   = digit { digit } ;
Hex       = "0x" hex_digit { hex_digit } ;
Number    = Decimal | Hex ;
String    = "\"" { Character } "\"" ;
Character = character
            | ["\\"] anycharacter ;
```

Example:

```eldritch-trace
# Define the serialised Ledger and Signals collected
# ============================================================
ledger  = "../../aetherus-scene/underwater-paper/out/simulation_ledger.json"
signals = "../../aetherus-scene/underwater-paper/out/multispectral/photon_collector_spad_sensor.csv"

# Define material and surface identifiers we want to match for
# ============================================================

src water_id = any[Mat("seawater"), MatSurf("Water:Water_material")]
src glass_id = any[Mat("glass"), any[MatSurf("Tank:Tank_material"), MatSurf("Tank-Water:Tank_material")]]
src toy_id   = Surf("TargetToy")
src tube_id  = Surf("TargetTube")
src air_id   = Mat(0)

# Define encoding patterns to match for
# =====================================

pattern water_scatter     = MCRT | Material  | Elastic | X            | water_id
pattern water_backscatter = MCRT | Material  | Elastic | X | Backward | water_id
pattern glass_interf      = MCRT | Interface | X                      | glass_id

pattern toy_surf          = MCRT | Reflector | X                      | toy_id
pattern tube_surf         = MCRT | Reflector | X                      | tube_id


# Define sequences of events in the photon history to search for
# ==============================================================
sequence seq_water_backscatter = seq[
    Emission | X | Light(0),
    * X,
    water_scatter,
    * X,
    Detection | X | Detector(0),
]

# Note how rule tries to validate all conditions enumerated
rule backscatter = {
    ! any[tube_surf, toy_surf],
    seq[
        Emission | X | Light(0),
        * X,
        + water_scatter,
        * X,
        Detection | X | Detector(0),
    ],
}

rule toy_or_tube_detect = {
    any[toy_surf, tube_surf],
    seq[
        Emission | X | Light(0),
        * X,
        + water_scatter,
        * X,
        Detection | X | Detector(0),
    ],
}
```
