use std::str::FromStr;
use anyhow::{anyhow, Error};

#[derive(Debug)]
pub struct BitsRange(usize, usize); // (end, start) - "end:start" Verilog style

impl BitsRange {
    pub fn size(&self) -> usize {
        self.0 - self.1 + 1
    }
}

impl FromStr for BitsRange {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid bits range format: {}", s));
        }
        let end = parts[0]
            .parse::<usize>()
            .map_err(|e| anyhow!("Invalid start bit: {}", e))?;
        let start = parts[1]
            .parse::<usize>()
            .map_err(|e| anyhow!("Invalid end bit: {}", e))?;
        Ok(BitsRange(end, start))
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BitsMatch {
    pub mask: u32,
    pub value: u32,
}

impl BitsMatch {
    pub fn check(&self, event_encoded: u32) -> bool {
        self.mask & event_encoded == self.value
    }
}

impl From<aetherus_events::filter::BitsMatch> for BitsMatch {
    fn from(other: aetherus_events::filter::BitsMatch) -> Self {
        BitsMatch { mask: other.mask, value: other.value }
    }
}

impl Into<aetherus_events::filter::BitsMatch> for BitsMatch {
    fn into(self) -> aetherus_events::filter::BitsMatch {
        aetherus_events::filter::BitsMatch { mask: self.mask, value: self.value }
    }
}

impl std::fmt::Debug for BitsMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BitsMatch {{ mask: 0x{:x}, value: 0x{:x} }}", self.mask, self.value)
    }
}

impl BitsMatch {
    pub fn parse(bits_range: &BitsRange, input: &str) -> Self {
        let num_bits = bits_range.0 - bits_range.1 + 1;
        let lsb_pos = bits_range.1;
        let mask = ((1 << num_bits) - 1) << lsb_pos;
        // input can be:
        // - "0b001011" (binary)
        // - "0x2F" (hex)
        // - "_"/"X" (don't care)
        match input.trim() {
            "X" | "_" => BitsMatch{mask: 0, value: 0},
            s => {
                if let Some(rest) = s.strip_prefix("0b") {
                    // Binary value can also be "0b01xxxxx"
                    let mut mask = 0;
                    let mut value = 0;
                    for c in rest.chars() {
                        match c {
                            'x' => {
                                mask = mask << 1;
                                value = value << 1;
                            }
                            '0' | '1' => {
                                mask = (mask << 1) | 1;
                                value = (value << 1) | if c == '1' { 1 } else { 0 };
                            }
                            _ => panic!("Invalid character '{}' in binary literal '{}'", c, s),
                        }
                    }
                    BitsMatch{ mask: mask << lsb_pos, value: value << lsb_pos }
                } else if let Some(rest) = s.strip_prefix("0x") {
                    let value = u32::from_str_radix(rest, 16).unwrap() << lsb_pos;
                    BitsMatch { mask, value }
                } else {
                    let value = s.parse::<u32>().unwrap() << lsb_pos;
                    BitsMatch{ mask, value }
                }
            }
        }
    }

    pub fn combine(&self, other: &BitsMatch) -> BitsMatch {
        assert!(self.mask & other.mask == 0 || (self.mask & other.mask & self.value) == (self.mask & other.mask & other.value), "Cannot combine BitsMatch with ambiguous values in overlapping masks");
        BitsMatch {
            mask: self.mask | other.mask,
            value: (self.mask & self.value) | (other.mask & other.value),
        }
    }
}
