use verdantgolem_data::packet::serverbound::play::SPECTATE_ENTITY;
use verdantgolem_macros::java_packet;

use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use verdantgolem_util::version::JavaMinecraftVersion;

#[java_packet(SPECTATE_ENTITY)]
pub struct SSpectateEntity {
    pub target: uuid::Uuid,
}

impl<'a> ServerPacket<'a> for SSpectateEntity {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            target: bytebuf.get_uuid()?,
        })
    }
}

impl crate::ClientPacket for SSpectateEntity {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_uuid(&self.target)?;
        Ok(())
    }
}
