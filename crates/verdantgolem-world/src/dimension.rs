use std::{path::PathBuf, sync::Arc};

use verdantgolem_config::world::LevelConfig;
use verdantgolem_data::dimension::Dimension;

use crate::level::Level;

#[must_use]
pub fn into_level(
    dimension: Dimension,
    level_config: &LevelConfig,
    base_directory: PathBuf,
    seed: i64,
) -> Arc<Level> {
    Level::from_root_folder(level_config, base_directory, seed, dimension)
}
