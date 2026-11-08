//! Warez binaries: every program on your HDD is 32 bytes you can open in the hex editor.
//! A program's stats are whatever its bytes say, as long as the binary passes [`check`].
//!
//! ```text
//! 00-01  'W' 'Z'      magic
//! 02     01           format version
//! 03     opcode       01 BRK  02 SHD  03 REP  04 SCR  05 STN
//! 04     power
//! 05     vs class     00 any  01 barrier  02 code gate  03 sentry   (breakers only)
//! 06     cpu cost     1..=MAX_CPU
//! 07     reserved     must be 00 (was the RAM cost, before programs stopped loading)
//! 08     opt level    raised only by optimizer fragments
//! 09-0A  signature    u16 LE, signs the opt level with the run's key
//! 0B-0F  reserved     free scratch space, ignored
//! 10-1E  name         ASCII, zero padded
//! 1F     checksum     sum of bytes 00-1E, mod 256
//! ```

use crate::data::{Effect, IceClass, WarezDef};
use crate::tuning::*;

pub const LEN: usize = 32;
pub type Bin = [u8; LEN];

pub const OFF_OPCODE: usize = 0x03;
pub const OFF_POWER: usize = 0x04;
pub const OFF_VS: usize = 0x05;
pub const OFF_CPU: usize = 0x06;
/// Byte 07: reserved, must be zero.
pub const OFF_SPARE: usize = 0x07;
pub const OFF_OPT: usize = 0x08;
pub const OFF_SIG: usize = 0x09;
pub const OFF_NAME: usize = 0x10;
pub const NAME_LEN: usize = 15;
pub const OFF_CHECKSUM: usize = 0x1F;

const MAGIC: [u8; 3] = [b'W', b'Z', 0x01];

/// What a byte offset means, for the editor's colouring and inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Magic,
    Opcode,
    Power,
    Vs,
    Cpu,
    /// Byte 07: must be zero.
    Spare,
    Opt,
    Signature,
    Reserved,
    Name,
    Checksum,
}

impl Field {
    pub fn at(offset: usize) -> Field {
        match offset {
            0..=2 => Field::Magic,
            OFF_OPCODE => Field::Opcode,
            OFF_POWER => Field::Power,
            OFF_VS => Field::Vs,
            OFF_CPU => Field::Cpu,
            OFF_SPARE => Field::Spare,
            OFF_OPT => Field::Opt,
            0x09..=0x0A => Field::Signature,
            0x0B..=0x0F => Field::Reserved,
            OFF_CHECKSUM => Field::Checksum,
            _ => Field::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Field::Magic => "magic",
            Field::Opcode => "opcode",
            Field::Power => "power",
            Field::Vs => "vs class",
            Field::Cpu => "cpu cost",
            Field::Spare => "reserved",
            Field::Opt => "opt level",
            Field::Signature => "signature",
            Field::Reserved => "reserved",
            Field::Name => "name",
            Field::Checksum => "checksum",
        }
    }
}

/// A program as the game runs it, decoded from a valid binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub name: String,
    pub effect: Effect,
    pub cpu: u32,
    pub opt: u32,
}

impl Spec {
    /// Lowercase filename shown in the warez tree.
    pub fn file(&self) -> String {
        format!("{}.exe", self.name.to_ascii_lowercase())
    }
}

pub fn checksum(b: &Bin) -> u8 {
    b[..OFF_CHECKSUM]
        .iter()
        .fold(0u8, |a, &x| a.wrapping_add(x))
}

/// The signature for an opt level under a run's key.
pub fn signature(key: u64, opt: u8) -> u16 {
    let mut z = key ^ 0xC0DE_D00D ^ (u64::from(opt) << 32).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as u16
}

pub fn read_sig(b: &Bin) -> u16 {
    u16::from_le_bytes([b[OFF_SIG], b[OFF_SIG + 1]])
}

/// Re-signs the opt level and fixes the checksum. Only trusted tools (fragments, the factory) do this.
pub fn seal(b: &mut Bin, key: u64) {
    let sig = signature(key, b[OFF_OPT]).to_le_bytes();
    b[OFF_SIG] = sig[0];
    b[OFF_SIG + 1] = sig[1];
    b[OFF_CHECKSUM] = checksum(b);
}

pub fn opcode_name(op: u8) -> &'static str {
    match op {
        0x01 => "BRK break",
        0x02 => "SHD shield",
        0x03 => "REP repair",
        0x04 => "SCR scrub trace",
        0x05 => "STN stun",
        _ => "??? unknown",
    }
}

pub fn vs_name(vs: u8) -> &'static str {
    match vs {
        0 => "any",
        1 => "barrier",
        2 => "code gate",
        3 => "sentry",
        _ => "???",
    }
}

fn vs_byte(vs: Option<IceClass>) -> u8 {
    match vs {
        None => 0,
        Some(IceClass::Barrier) => 1,
        Some(IceClass::CodeGate) => 2,
        Some(IceClass::Sentry) => 3,
    }
}

/// Encodes a factory program and seals it.
pub fn encode(def: &WarezDef, key: u64) -> Bin {
    let mut b = [0u8; LEN];
    b[..3].copy_from_slice(&MAGIC);
    let (op, power, vs) = match def.effect {
        Effect::Break { power, vs } => (1, power, vs_byte(vs)),
        Effect::Shield(n) => (2, n, 0),
        Effect::Repair(n) => (3, n, 0),
        Effect::Scrub(n) => (4, n, 0),
        Effect::Stun => (5, 0, 0),
    };
    b[OFF_OPCODE] = op;
    b[OFF_POWER] = power.min(255) as u8;
    b[OFF_VS] = vs;
    b[OFF_CPU] = def.cpu.min(255) as u8;
    for (i, ch) in def
        .name
        .bytes()
        .filter(u8::is_ascii_graphic)
        .take(NAME_LEN)
        .enumerate()
    {
        b[OFF_NAME + i] = ch;
    }
    seal(&mut b, key);
    b
}

/// Budget points an effect costs.
pub fn cost(effect: Effect) -> u32 {
    match effect {
        Effect::Break { power, .. } => power,
        Effect::Shield(n) | Effect::Scrub(n) => n.div_ceil(2),
        Effect::Repair(n) => n,
        Effect::Stun => COST_STUN,
    }
}

/// Budget points a program with these costs may spend.
pub fn budget(cpu: u32, opt: u32) -> u32 {
    BUDGET_PER_CPU * cpu + BUDGET_PER_OPT * opt
}

/// Hex text for saving a binary.
pub fn to_hex(b: &Bin) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Parses hex text back into a binary (exactly 32 bytes).
pub fn from_hex(s: &str) -> Option<Bin> {
    let s = s.trim();
    if s.len() != LEN * 2 || !s.is_ascii() {
        return None;
    }
    let mut b = [0u8; LEN];
    for (i, byte) in b.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(b)
}

/// Validates a binary. On failure, every fault is listed, most basic first.
pub fn check(b: &Bin, key: u64) -> Result<Spec, Vec<String>> {
    let mut faults = Vec::new();
    if b[..3] != MAGIC {
        faults.push("bad magic: header must be 57 5A 01 (\"WZ\" v1)".to_string());
    }
    let power = u32::from(b[OFF_POWER]);
    let vs = match b[OFF_VS] {
        0 => None,
        1 => Some(IceClass::Barrier),
        2 => Some(IceClass::CodeGate),
        3 => Some(IceClass::Sentry),
        v => {
            faults.push(format!("vs class {v:02X} unknown (00-03)"));
            None
        }
    };
    let effect = match b[OFF_OPCODE] {
        1 => Some(Effect::Break { power, vs }),
        2 => Some(Effect::Shield(power)),
        3 => Some(Effect::Repair(power)),
        4 => Some(Effect::Scrub(power)),
        5 => Some(Effect::Stun),
        op => {
            faults.push(format!("illegal opcode {op:02X} (01-05)"));
            None
        }
    };
    if b[OFF_VS] != 0 && b[OFF_OPCODE] != 1 {
        faults.push("vs class is only valid on BRK".to_string());
    }
    if b[OFF_OPCODE] != 5 && !(1..=MAX_POWER).contains(&power) {
        faults.push(format!("power {power} out of range (1-{MAX_POWER})"));
    }
    let cpu = u32::from(b[OFF_CPU]);
    let opt = u32::from(b[OFF_OPT]);
    if !(1..=MAX_CPU).contains(&cpu) {
        faults.push(format!("cpu cost {cpu} out of range (1-{MAX_CPU})"));
    }
    if b[OFF_SPARE] != 0 {
        faults.push(format!(
            "byte 07 is reserved: must be 00, not {:02X}",
            b[OFF_SPARE]
        ));
    }
    if opt > MAX_OPT {
        faults.push(format!("opt level {opt} out of range (0-{MAX_OPT})"));
    }
    if read_sig(b) != signature(key, b[OFF_OPT]) {
        faults.push("signature does not match opt level".to_string());
    }
    let name_bytes = &b[OFF_NAME..OFF_NAME + NAME_LEN];
    let end = name_bytes.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
    let (name, tail) = name_bytes.split_at(end);
    if name.is_empty() {
        faults.push("name is empty".to_string());
    } else if !name.iter().all(u8::is_ascii_graphic) {
        faults.push("name must be printable ASCII with no spaces".to_string());
    } else if tail.iter().any(|&c| c != 0) {
        faults.push("name must be zero padded".to_string());
    }
    if let Some(e) = effect {
        let (c, bud) = (cost(e), budget(cpu, opt));
        if c > bud {
            faults.push(format!(
                "over budget: effect costs {c}, cpu/opt allow {bud}"
            ));
        }
    }
    let sum = checksum(b);
    if b[OFF_CHECKSUM] != sum {
        faults.push(format!(
            "checksum mismatch: stored {:02X}, computed {sum:02X}",
            b[OFF_CHECKSUM]
        ));
    }
    match effect {
        Some(effect) if faults.is_empty() => Ok(Spec {
            name: String::from_utf8_lossy(name).into_owned(),
            effect,
            cpu,
            opt,
        }),
        _ => Err(faults),
    }
}
