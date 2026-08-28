use verdantgolem_data::packet::serverbound::play::PING_REQUEST;
use verdantgolem_macros::java_packet;

use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use verdantgolem_util::version::JavaMinecraftVersion;

#[java_packet(PING_REQUEST)]
pub struct SPlayPingRequest {
    pub payload: i64,
}

impl<'a> ServerPacket<'a> for SPlayPingRequest {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            payload: bytebuf.get_i64_be()?,
        })
    }
}

impl crate::ClientPacket for SPlayPingRequest {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_i64_be(self.payload)?;
        Ok(())
    }
}
