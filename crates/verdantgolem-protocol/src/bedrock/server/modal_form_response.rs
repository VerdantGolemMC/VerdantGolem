// Last verified for v2169

use crate::codec::var_uint::VarUInt;
use crate::serial::{PacketRead, PacketReadSlice};
use std::borrow::Cow;
use verdantgolem_macros::packet;

#[derive(Debug, PacketRead, PacketReadSlice)]
#[packet(101)]
pub struct SModalFormResponse<'a> {
    pub form_id: VarUInt,
    pub json_response: Option<Cow<'a, str>>,

    // TODO: enum ModalFormCancelReason
    pub form_cancel_reason: Option<u8>,
}
