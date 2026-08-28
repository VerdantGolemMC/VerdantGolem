#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub async fn handle_known_packs(&mut self) {
        let version = self.version.load();
        if version.supports_configuration_state() {
            self.send_packet_now(&CFeatureFlags::new(&["minecraft:vanilla".to_string()]))
                .await;
            let registry = verdantgolem_data::registry::Registry::get_synced(version);
            for reg in &registry {
                self.send_packet_now(&CRegistryData::new(&reg.registry_id, &reg.registry_entries))
                    .await;
            }
        }
        let mut tags = Vec::new();
        for &key in verdantgolem_data::tag::RegistryKey::NETWORK_KEYS {
            if verdantgolem_data::tag::get_registry_key_tags(version, key)
                .is_some_and(|map| !map.is_empty())
            {
                tags.push(key);
            }
        }
        self.send_packet_now(&CUpdateTags::new(&tags)).await;
        self.send_packet_now(&CFinishConfig).await;
    }
}
