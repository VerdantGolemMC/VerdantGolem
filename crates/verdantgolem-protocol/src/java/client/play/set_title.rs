use verdantgolem_data::packet::clientbound::play::SET_TITLE_TEXT;
use verdantgolem_util::text::TextComponent;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use verdantgolem_macros::java_packet;
use verdantgolem_util::version::JavaMinecraftVersion;

#[java_packet(SET_TITLE_TEXT)]
pub struct CTitleText<'a> {
    pub title: &'a TextComponent,
}

impl<'a> CTitleText<'a> {
    #[must_use]
    pub const fn new(title: &'a TextComponent) -> Self {
        Self { title }
    }
}

impl ClientPacket for CTitleText<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.title, version)
    }
}
