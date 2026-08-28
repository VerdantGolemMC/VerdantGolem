// Last verified for v2169

use verdantgolem_macros::packet;

use crate::{codec::var_int::VarInt, serial::PacketRead};

#[derive(PacketRead, Debug)]
#[packet(69)]
pub struct SRequestChunkRadius {
    pub chunk_radius: VarInt,
    pub max_chunk_radius: u8,
}
