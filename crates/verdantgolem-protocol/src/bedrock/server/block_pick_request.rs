// Last verified for v2169

use verdantgolem_macros::packet;
use verdantgolem_util::math::position::BlockPos;

use crate::serial::PacketRead;

#[derive(Debug, PacketRead)]
#[packet(34)]
pub struct SBlockPickRequest {
    pub position: BlockPos,
    pub with_data: bool,
    pub max_slots: u8,
}
