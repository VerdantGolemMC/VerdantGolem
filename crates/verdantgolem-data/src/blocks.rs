use crate::{
    BlockState, BlockStateId,
    tag::{RegistryKey, Tag, Taggable},
};
use std::hash::{Hash, Hasher};
use verdantgolem_util::{
    loot_table::LootTable,
    math::{experience::Experience, position::BlockPos, vector3::Vector3},
    random::hash_block_pos,
    resource_location::{FromResourceLocation, ResourceLocation, ToResourceLocation},
};

/// Represents the static definition of a Minecraft block type.
///
/// This struct contains the base properties shared by all instances of a block
/// Data-driven attributes like `hardness` and `blast_resistance` are defined here,
/// while specific orientations or variations are stored in the associated `BlockState`.
#[derive(Debug, Clone)]
pub struct Block {
    /// The numeric ID used for internal registry mapping.
    pub id: BlockId,
    /// The unique namespaced ID (e.g., "`diamond_ore`").
    pub name: &'static str,
    /// How hard the block is to break. A value of -1.0 indicates an unbreakable block (e.g., Bedrock).
    pub hardness: f32,
    /// The block's resistance to explosions.
    pub blast_resistance: f32,
    pub map_color: u8,
    /// The friction coefficient. Default is 0.6; Ice is 0.98.
    pub slipperiness: f32,
    /// How much this block affects the speed of an entity walking on it (e.g., Soul Sand).
    pub velocity_multiplier: f32,
    /// How much this block affects an entity's jump height (e.g., Honey Blocks).
    pub jump_velocity_multiplier: f32,
    /// The ID of the item form of this block, used for inventory and drops.
    pub item_id: u16,
    /// The initial state of the block when placed without extra data.
    pub default_state: &'static BlockState,
    /// A list of all possible valid states (properties like rotation, waterlogged, etc.) for this block.
    pub states: &'static [BlockState],
    /// Fire behavior settings. If `None`, the block is not flammable.
    pub flammable: Option<Flammable>,
    /// Defines the items dropped when this block is destroyed.
    pub loot_table: Option<LootTable>,
    /// Defines the amount of XP dropped when the block is mined (e.g., Coal or Diamond).
    pub experience: Option<Experience>,
}

/// Helper struct to ensure the validity of BlockIds parsed from external sources.
/// Every [`BlockId`] is guaranteed to correspond to a valid [`Block`].
///
/// Also enables [`Block`]-type pattern matching, even in const contexts:
/// ```rs
/// const fn to_waxed(block: &'static Block) -> Option<&'static Block> {
///     match block.id {
///         BlockId::COPPER_BLOCK => Some(Block::WAXED_COPPER_BLOCK),
///         //...
///         _ => None
///     }
/// }
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BlockId(u16);

impl PartialEq<BlockId> for Block {
    fn eq(&self, other: &BlockId) -> bool {
        self.id == *other
    }
}

impl PartialEq<Block> for BlockId {
    fn eq(&self, other: &Block) -> bool {
        *self == other.id
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Block {}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Taggable for Block {
    #[inline]
    fn tag_key() -> RegistryKey {
        RegistryKey::Block
    }

    #[inline]
    fn registry_key(&self) -> &str {
        self.name
    }

    #[inline]
    fn registry_id(&self) -> u16 {
        self.id.as_u16()
    }
}

impl ToResourceLocation for &'static Block {
    fn to_resource_location(&self) -> ResourceLocation {
        format!("minecraft:{}", self.name)
    }
}

impl FromResourceLocation for &'static Block {
    fn from_resource_location(resource_location: &ResourceLocation) -> Option<Self> {
        Block::from_registry_key(
            resource_location
                .strip_prefix("minecraft:")
                .unwrap_or(resource_location),
        )
    }
}

impl Block {
    #[must_use]
    pub const fn get_speed_factor(&self) -> f32 {
        self.velocity_multiplier
    }

    #[must_use]
    pub const fn get_jump_velocity_multiplier(&self) -> f32 {
        self.jump_velocity_multiplier
    }

    pub(crate) fn shape_offset_delta(&self, pos: &BlockPos) -> Vector3<f64> {
        let Some(shape_offset) = self.shape_offset() else {
            return Vector3::new(0.0, 0.0, 0.0);
        };

        let seed = hash_block_pos(pos.0.x, 0, pos.0.z) as u64;
        let max_horizontal = f64::from(shape_offset.max_horizontal);
        let x = (f64::from((seed & 15) as f32 / 15.0) - 0.5) * 0.5;
        let x = x.clamp(-max_horizontal, max_horizontal);
        let z = (f64::from(((seed >> 8) & 15) as f32 / 15.0) - 0.5) * 0.5;
        let z = z.clamp(-max_horizontal, max_horizontal);
        let y = match shape_offset.offset_type {
            ShapeOffsetType::Xz => 0.0,
            ShapeOffsetType::Xyz => {
                (f64::from(((seed >> 4) & 15) as f32 / 15.0) - 1.0)
                    * f64::from(shape_offset.max_vertical)
            }
        };

        // Extracted shapes are sampled at BlockPos::ZERO, where vanilla uses the
        // negative horizontal limits and, for XYZ offsets, the negative vertical
        // limit. Return only the delta from that sample.
        Vector3::new(
            x + max_horizontal,
            y + shape_offset.offset_type.origin_y(shape_offset.max_vertical),
            z + max_horizontal,
        )
    }

    #[must_use]
    pub fn is_waterlogged(&self, id: BlockStateId) -> bool {
        self.properties(id).is_some_and(|properties| {
            properties
                .to_props()
                .into_iter()
                .any(|(key, value)| key == "waterlogged" && value == "true")
        })
    }

    /// Returns a new [`BlockState`] reference for the given [`BlockStateId`] with the
    /// `waterlogged` property forced to `true` if the block supports that
    /// property.  If the state is already waterlogged or the block does not
    /// expose a `waterlogged` property then `None` is returned.
    #[must_use]
    pub fn with_waterlogged(&self, id: BlockStateId) -> Option<&'static BlockState> {
        // Check if already waterlogged
        if self.is_waterlogged(id) {
            return Some(BlockState::from_id(id));
        }

        // Modify the property list if available
        if let Some(props_source) = self.properties(id) {
            let mut props: Vec<(&str, &str)> = props_source
                .to_props()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect();

            // Look for an existing waterlogged key or add one
            if let Some(idx) = props.iter().position(|(k, _)| *k == "waterlogged") {
                props[idx] = ("waterlogged", "true");
            } else {
                props.push(("waterlogged", "true"));
            }

            let new_state_id = self.from_properties(&props).to_state_id(self);
            return Some(BlockState::from_id(new_state_id));
        }

        None
    }

    /// Returns whether this block is solid (based on default state)
    #[must_use]
    pub const fn is_solid(&self) -> bool {
        self.default_state.is_solid()
    }

    /// Returns whether this block is air (based on default state)
    #[must_use]
    pub const fn is_air(&self) -> bool {
        self.default_state.is_air()
    }

    #[must_use]
    pub const fn mirror(
        &self,
        id: BlockStateId,
        _mirror: crate::block_rotation::Mirror,
    ) -> &'static BlockState {
        BlockState::from_id(id)
    }

    #[must_use]
    pub fn rotate(
        &self,
        id: BlockStateId,
        rotation: crate::block_rotation::Rotation,
    ) -> &'static BlockState {
        use crate::block_rotation::Rotation;

        if rotation == Rotation::None {
            return BlockState::from_id(id);
        }

        let Some(properties) = self.properties(id) else {
            return BlockState::from_id(id);
        };
        let mut changed = false;
        let mut props = properties
            .to_props()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();

        for (key, value) in &mut props {
            let rotated = match key.as_str() {
                "facing" if matches!(value.as_str(), "north" | "south" | "east" | "west") => {
                    Some(rotation.rotate_facing(value).to_owned())
                }
                "axis" if matches!(value.as_str(), "x" | "z") => {
                    Some(rotation.rotate_axis(value).to_owned())
                }
                "rotation" => value
                    .parse::<i32>()
                    .ok()
                    .map(|value| rotation.rotate_block_rotation(value).to_string()),
                "shape" => rotate_rail_shape(value, rotation).map(str::to_owned),
                "orientation" => rotate_orientation(value, rotation),
                _ => None,
            };
            if let Some(rotated) = rotated
                && rotated != *value
            {
                *value = rotated;
                changed = true;
            }
        }

        // Connection-like blocks encode directions in property names rather
        // than values (for example `north=true` on fences and `west=low` on
        // walls), so rotate the four keys as one permutation as well.
        for (key, _) in &mut props {
            if matches!(key.as_str(), "north" | "south" | "east" | "west") {
                let rotated = rotation.rotate_facing(key).to_owned();
                if rotated != *key {
                    *key = rotated;
                    changed = true;
                }
            }
        }

        if !changed {
            return BlockState::from_id(id);
        }
        let borrowed = props
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        BlockState::from_id(self.from_properties(&borrowed).to_state_id(self))
    }
}

fn rotate_orientation(
    orientation: &str,
    rotation: crate::block_rotation::Rotation,
) -> Option<String> {
    fn rotate_component(
        component: &str,
        rotation: crate::block_rotation::Rotation,
    ) -> Option<&str> {
        match component {
            "north" | "south" | "east" | "west" => Some(rotation.rotate_facing(component)),
            "up" | "down" => Some(component),
            _ => None,
        }
    }

    let (first, second) = orientation.split_once('_')?;
    Some(format!(
        "{}_{}",
        rotate_component(first, rotation)?,
        rotate_component(second, rotation)?
    ))
}

fn rotate_rail_shape(
    shape: &str,
    rotation: crate::block_rotation::Rotation,
) -> Option<&'static str> {
    use crate::block_rotation::Rotation;

    let steps = match rotation {
        Rotation::None => 0,
        Rotation::Clockwise90 => 1,
        Rotation::Rotate180 => 2,
        Rotation::CounterClockwise90 => 3,
    };
    let mut shape = match shape {
        "north_south" => "north_south",
        "east_west" => "east_west",
        "ascending_east" => "ascending_east",
        "ascending_west" => "ascending_west",
        "ascending_north" => "ascending_north",
        "ascending_south" => "ascending_south",
        "south_east" => "south_east",
        "south_west" => "south_west",
        "north_west" => "north_west",
        "north_east" => "north_east",
        _ => return None,
    };
    for _ in 0..steps {
        shape = match shape {
            "north_south" => "east_west",
            "east_west" => "north_south",
            "ascending_east" => "ascending_south",
            "ascending_south" => "ascending_west",
            "ascending_west" => "ascending_north",
            "ascending_north" => "ascending_east",
            "south_east" => "south_west",
            "south_west" => "north_west",
            "north_west" => "north_east",
            "north_east" => "south_east",
            _ => return None,
        };
    }
    Some(shape)
}

#[derive(Clone, Copy)]
pub(crate) enum ShapeOffsetType {
    Xz,
    Xyz,
}

#[derive(Clone, Copy)]
pub(crate) struct ShapeOffset {
    pub offset_type: ShapeOffsetType,
    pub max_horizontal: f32,
    pub max_vertical: f32,
}

impl ShapeOffsetType {
    fn origin_y(self, max_vertical: f32) -> f64 {
        match self {
            Self::Xz => 0.0,
            Self::Xyz => f64::from(max_vertical),
        }
    }
}

impl BlockId {
    // depends on generated impl:
    // pub(crate) const BLOCK_COUNT: u16;

    /// The total count of all registered blocks.
    pub const COUNT: u16 = Self::BLOCK_COUNT;

    // SAFETY: There must never be a BlockId where self.0 >= BlockId::BLOCK_COUNT

    #[inline]
    #[must_use]
    pub const fn new(inner: u16) -> Option<Self> {
        if inner < Self::BLOCK_COUNT {
            return Some(Self(inner));
        }
        None
    }

    #[inline]
    #[must_use]
    pub const fn new_or_air(inner: u16) -> Self {
        if inner < Self::BLOCK_COUNT {
            return Self(inner);
        }
        Self::AIR
    }

    #[inline]
    #[must_use]
    pub const fn to_block(self) -> &'static Block {
        Block::from_id(self)
    }

    #[inline(always)]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[inline]
    #[must_use]
    pub fn has_tag(self, tag: Tag) -> bool {
        tag.1.contains(&self.0)
    }
}

impl From<BlockId> for u16 {
    #[inline]
    fn from(value: BlockId) -> Self {
        value.as_u16()
    }
}

impl Default for BlockId {
    #[inline]
    fn default() -> Self {
        Self::AIR
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BlockId({} = \"{}\")",
            self.0,
            Block::from_id(*self).name
        )
    }
}

#[derive(Clone, Debug)]
pub struct Flammable {
    pub spread_chance: u8,
    pub burn_chance: u8,
}

#[cfg(test)]
mod tests {
    use super::{Block, BlockId, ShapeOffsetType};
    use crate::block_rotation::Rotation;
    use verdantgolem_util::math::position::BlockPos;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
    }

    #[test]
    fn shape_offset_registry_matches_vanilla_26_2() {
        let mut xz = 0;
        let mut xyz = 0;

        for raw_id in 0..BlockId::BLOCK_COUNT {
            match Block::from_id(BlockId::new(raw_id).unwrap())
                .shape_offset()
                .map(|offset| offset.offset_type)
            {
                Some(ShapeOffsetType::Xz) => xz += 1,
                Some(ShapeOffsetType::Xyz) => xyz += 1,
                None => {}
            }
        }

        assert_eq!(xz, 34);
        assert_eq!(xyz, 5);
    }

    #[test]
    fn shape_offset_limits_match_vanilla_26_2() {
        let bamboo = Block::BAMBOO.shape_offset().unwrap();
        assert_eq!(bamboo.max_horizontal, 0.25);
        assert_eq!(bamboo.max_vertical, 0.2);

        let pointed_dripstone = Block::POINTED_DRIPSTONE.shape_offset().unwrap();
        assert_eq!(pointed_dripstone.max_horizontal, 0.125);

        let small_dripleaf = Block::SMALL_DRIPLEAF.shape_offset().unwrap();
        assert_eq!(small_dripleaf.max_vertical, 0.1);
    }

    #[test]
    fn shape_offset_delta_matches_vanilla_coordinate_hash() {
        let origin = BlockPos::new(0, 64, 0);
        let positive_extreme = BlockPos::new(-18, 64, -7);

        let origin_delta = Block::BAMBOO.shape_offset_delta(&origin);
        assert_eq!(origin_delta.x, 0.0);
        assert_eq!(origin_delta.y, 0.0);
        assert_eq!(origin_delta.z, 0.0);

        let bamboo_delta = Block::BAMBOO.shape_offset_delta(&positive_extreme);
        assert_eq!(bamboo_delta.x, 0.5);
        assert_eq!(bamboo_delta.y, 0.0);
        assert_eq!(bamboo_delta.z, 0.5);

        let speleothem_delta = Block::POINTED_DRIPSTONE.shape_offset_delta(&positive_extreme);
        assert_eq!(speleothem_delta.x, 0.25);
        assert_eq!(speleothem_delta.y, 0.0);
        assert_eq!(speleothem_delta.z, 0.25);
        assert_eq!(
            Block::SULFUR_SPIKE.shape_offset_delta(&positive_extreme),
            speleothem_delta
        );

        let xyz_delta = Block::SHORT_GRASS.shape_offset_delta(&positive_extreme);
        assert_eq!(xyz_delta.x, 0.5);
        assert_close(xyz_delta.y, 0.08);
        assert_eq!(xyz_delta.z, 0.5);

        assert_eq!(Block::STONE.shape_offset_delta(&positive_extreme).x, 0.0);
    }

    #[test]
    fn rotates_directional_and_axis_block_states() {
        let dispenser = Block::DISPENSER;
        let north = dispenser.from_properties(&[("facing", "north"), ("triggered", "false")]);
        let rotated = dispenser.rotate(north.to_state_id(&dispenser), Rotation::CounterClockwise90);
        let rotated_props = dispenser.properties(rotated.id).unwrap().to_props();
        assert!(rotated_props.contains(&("facing", "west")));

        let oak_log = Block::OAK_LOG;
        let x_axis = oak_log.from_properties(&[("axis", "x")]);
        let rotated = oak_log.rotate(x_axis.to_state_id(&oak_log), Rotation::Clockwise90);
        let rotated_props = oak_log.properties(rotated.id).unwrap().to_props();
        assert!(rotated_props.contains(&("axis", "z")));
    }

    #[test]
    fn rotates_connection_keys_and_orientation_values() {
        let fence = Block::OAK_FENCE;
        let north = fence.from_properties(&[
            ("north", "true"),
            ("east", "false"),
            ("south", "false"),
            ("west", "false"),
        ]);
        let rotated = fence.rotate(north.to_state_id(&fence), Rotation::Clockwise90);
        let rotated_props = fence.properties(rotated.id).unwrap().to_props();
        assert!(rotated_props.contains(&("east", "true")));
        assert!(rotated_props.contains(&("north", "false")));

        let jigsaw = Block::JIGSAW;
        let north_up = jigsaw.from_properties(&[("orientation", "north_up")]);
        let rotated = jigsaw.rotate(north_up.to_state_id(&jigsaw), Rotation::Clockwise90);
        let rotated_props = jigsaw.properties(rotated.id).unwrap().to_props();
        assert!(rotated_props.contains(&("orientation", "east_up")));
    }
}
