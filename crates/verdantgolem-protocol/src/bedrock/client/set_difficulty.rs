// Last verified for v2169

use verdantgolem_macros::packet;

use crate::{codec::var_uint::VarUInt, serial::PacketWrite};

#[derive(PacketWrite)]
#[packet(60)]
pub struct CSetDifficulty {
    pub difficulty: VarUInt,
}
