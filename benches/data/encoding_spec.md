# Encoding/Decoding Specification

> ![TODO]
> Move attribute annotation in the bits range instead of per field name
> definition.

## Generic encoding

| 31:28     | 27:24    | 23:16     | 15:0          |
|-----------|----------|-----------|---------------|
| _Reserved | Pipeline | EventType | SrcId {SrcId} |
| _         | _        | _         | _             |

## Supported encodings

| 27:24     | 23:16 | 15:0              |
|-----------|-------|-------------------|
| Emission  | _     | LightId {SrcId}   |
| 0b0001    | _     | _                 |
| MCRT      | _     | MatSurfId {SrcId} |
| 0b0011    | _     | _                 |
| Detection | _     | DetId {SrcId}     |
| 0b0101    | _     | _                 |

### MCRT Pipeline

| 27:24  | 23:22     | 21:16 | 15:0              |
|--------|-----------|-------|-------------------|
| MCRT   | Interface | _     | MatSurfId {SrcId} |
| 0b0011 | 0b00      | _     | _                 |
| MCRT   | Reflector | _     | SurfId {SrcId}    |
| 0b0011 | 0b01      | _     | _                 |
| MCRT   | Material  | _     | MatId {SrcId}     |
| 0b0011 | 0b10      | _     | _                 |
| MCRT   | _Custom   | _     | MatSurfId {SrcId} |
| 0b0011 | 0b11      | _     | _                 |

#### Interface

| 27:24  | 23:22     | 21:16       | 15:0              |
|--------|-----------|-------------|-------------------|
| MCRT   | Interface | Reflection  | MatSurfId {SrcId} |
| 0b0011 | 0b00      | 0b000000    | _                 |
| MCRT   | Interface | Refraction  | MatSurfId {SrcId} |
| 0b0011 | 0b00      | 0b000001    | _                 |
| MCRT   | Interface | ReEmittance | MatSurfId {SrcId} |
| 0b0011 | 0b00      | 0b000100    | _                 |
| MCRT   | Interface | Boundary    | MatSurfId {SrcId} |
| 0b0011 | 0b00      | 0b001000    | _                 |
| MCRT   | Interface | _Custom     | MatSurfId {SrcId} |
| 0b0011 | 0b00      | 0b1xxxxx    | _                 |

#### Reflector

| 27:24  | 23:22     | 21:16           | 15:0           |
|--------|-----------|-----------------|----------------|
| MCRT   | Reflector | Diffuse         | SurfId {SrcId} |
| 0b0011 | 0b01      | 0b00001x        | _              |
| MCRT   | Reflector | Specular        | SurfId {SrcId} |
| 0b0011 | 0b01      | 0b00010x        | _              |
| MCRT   | Reflector | Composite       | SurfId {SrcId} |
| 0b0011 | 0b01      | 0b00011x        | _              |
| MCRT   | Reflector | RetroReflective | SurfId {SrcId} 
| 0b0011 | 0b01      | 0b001xxx        | _              |
| MCRT   | Reflector | _Custom         | SurfId {SrcId} |
| 0b0011 | 0b01      | 0b1xxxxx        | _              |

#### Material

##### Absorption and Custom

| 27:24  | 23:22    | 21:20      | 19:16 | 15:0          |
|--------|----------|------------|-------|---------------|
| MCRT   | Material | Absorption | _     | MatId {SrcId} |
| 0b0011 | 0b10     | 0b00       | _     | _             |
| MCRT   | Material | _Custom    | _     | MatId {SrcId} |
| 0b0011 | 0b10     | 0b11       | _     | _             |

##### Scattering: Elastic & Inelastic
==TODO: Swap Elastic and inelastic values==

| 27:24  | 23:22    | 21:20     | 19:18 | 17:16       | 15:0          |
|--------|----------|-----------|-------|-------------|---------------|
| MCRT   | Material | Elastic   | _     | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b10      | _     | _           | _             |
| MCRT   | Material | Inelastic | _     | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b01      | _     | _           | _             |

###### Scattering: Elastic

| 27:24  | 23:22    | 21:20   | 19:18             | 17:16       | 15:0          |
|--------|----------|---------|-------------------|-------------|---------------|
| MCRT   | Material | Elastic | HenyeyGreenstein  | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b10    | 0b00              | _           | _             |
| MCRT   | Material | Elastic | Mie               | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b10    | 0b01              | _           | _             |
| MCRT   | Material | Elastic | Rayleigh          | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b10    | 0b10              | _           | _             |
| MCRT   | Material | Elastic | SphericalCDF      | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b10    | 0b11              | _           | _             |

###### Scattering: Inelastic

| 27:24  | 23:22    | 21:20     | 19:18        | 17:16       | 15:0          |
|--------|----------|-----------|--------------|-------------|---------------|
| MCRT   | Material | Inelastic | Raman        | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b01      | 0b00         | _           | _             |
| MCRT   | Material | Inelastic | Fluorescence | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b01      | 0b01         | _           | _             |
| MCRT   | Material | Inelastic | _Custom      | {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | 0b01      | 0b1x         | _           | _             |


###### Scattering Direction

| 27:24  | 23:22    | 21:18 | 17:16                | 15:0          |
|--------|----------|-------|----------------------|---------------|
| MCRT   | Material | _     | Unknown {Direction}  | MatId {SrcId} |
| 0b0011 | 0b10     | _     | 0b00                 | _             |
| MCRT   | Material | _     | Forward {Direction}  | MatId {SrcId} |
| 0b0011 | 0b10     | _     | 0b01                 | _             |
| MCRT   | Material | _     | Side {Direction}     | MatId {SrcId} |
| 0b0011 | 0b10     | _     | 0b10                 | _             |
| MCRT   | Material | _     | Backward {Direction} | MatId {SrcId} |
| 0b0011 | 0b10     | _     | 0b11                 | _             |
