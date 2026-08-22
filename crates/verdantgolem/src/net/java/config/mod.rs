use std::{num::NonZero, sync::Arc, sync::atomic::Ordering};

use crate::{
    entity::player::ChatMode,
    net::{
        PlayerConfig, can_not_join,
        java::{JavaClient, PacketHandlerResult},
    },
    server::Server,
};
use core::str;
use tracing::{debug, trace, warn};
use verdantgolem_data::registry::Registry;
use verdantgolem_protocol::{
    ConnectionState, KnownPack,
    java::{
        client::config::{CFeatureFlags, CFinishConfig, CKnownPacks, CRegistryData, CUpdateTags},
        server::config::{
            ResourcePackResponseResult, SClientInformationConfig, SConfigCookieResponse,
            SConfigResourcePack, SKeepAlive, SPluginMessage,
        },
    },
};
use verdantgolem_util::{Hand, text::TextComponent};

const BRAND_CHANNEL_PREFIX: &str = "minecraft:brand";

pub mod client_information;
pub mod config_acknowledged;
pub(super) use config_acknowledged::build_dimension_nbt;
pub mod cookie_response;
pub mod keep_alive;
pub mod known_packs;
pub mod plugin_message;
pub mod resource_pack;
