use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use verdantgolem_data::packet::serverbound::play::CONTAINER_CLOSE;
use verdantgolem_macros::java_packet;
use verdantgolem_util::version::JavaMinecraftVersion;

use crate::VarInt;

#[java_packet(CONTAINER_CLOSE)]
pub struct SCloseContainer {
    pub window_id: VarInt,
}

impl<'a> ServerPacket<'a> for SCloseContainer {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let window_id = bytebuf.get_container_id(version)?;

        Ok(Self { window_id })
    }
}

impl crate::ClientPacket for SCloseContainer {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;

        write.write_container_id(&self.window_id, version)?;

        Ok(())
    }
}
