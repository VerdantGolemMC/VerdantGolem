// Last verified for v2169

use crate::{codec::var_uint::VarUInt, serial::PacketWrite};
use verdantgolem_macros::packet;

#[packet(100)]
#[derive(PacketWrite)]
pub struct CModalFormRequest {
    pub form_id: VarUInt,
    pub form_ui_json: String,
}
