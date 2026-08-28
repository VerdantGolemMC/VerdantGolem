// Last verified for v2169

use verdantgolem_macros::packet;

use crate::serial::PacketRead;

#[derive(PacketRead)]
#[packet(193)]
pub struct SRequestNetworkSettings {
    #[serial(big_endian)]
    pub client_network_version: i32,
}
