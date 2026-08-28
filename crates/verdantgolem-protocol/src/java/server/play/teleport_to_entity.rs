use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use verdantgolem_data::packet::serverbound::play::TELEPORT_TO_ENTITY;
use verdantgolem_macros::java_packet;
use verdantgolem_util::version::JavaMinecraftVersion;

#[java_packet(TELEPORT_TO_ENTITY)]
pub struct STeleportToEntity {
    pub target: uuid::Uuid,
}

impl<'a> ServerPacket<'a> for STeleportToEntity {
    fn read(
        bytebuf: &mut &'a [u8],
        _protocol_version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        Ok(Self {
            target: bytebuf.get_uuid()?,
        })
    }
}

impl crate::ClientPacket for STeleportToEntity {
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
