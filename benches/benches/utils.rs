use ethnum::U256;
use fuel_core_types::fuel_asm::{
    op,
    Instruction,
    RegId,
};

/// Allocates a byte array from heap and initializes it. Then points `reg` to it.
fn aloc_bytearray<const S: usize>(reg: u8, v: [u8; S]) -> Vec<Instruction> {
    let mut ops = vec![op::movi(reg, S as u32), op::aloc(reg)];
    for (i, b) in v.iter().enumerate() {
        if *b != 0 {
            ops.push(op::movi(reg, *b as u32));
            ops.push(op::sb(RegId::HP, reg, i as u16));
        }
    }
    ops.push(op::move_(reg, RegId::HP));
    ops
}

pub fn make_u128(reg: u8, v: u128) -> Vec<Instruction> {
    aloc_bytearray(reg, v.to_be_bytes())
}

pub fn make_u256(reg: u8, v: U256) -> Vec<Instruction> {
    aloc_bytearray(reg, v.to_be_bytes())
}
