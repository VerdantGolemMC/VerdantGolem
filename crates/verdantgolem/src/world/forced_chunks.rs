//! Per-dimension, vanilla-compatible `/forceload` saved data.
//!
//! The owning world holds one mutex across each disk commit and ticket delta.
//! A read failure locks this store against mutations until the world is reloaded,
//! so malformed or unreadable saved data is never mistaken for an empty list.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use rustc_hash::FxHashSet;
use uuid::Uuid;
use verdantgolem_nbt::{
    MAX_ARRAY_LENGTH, NbtCompound,
    nbt_compress::{read_gzip_compound_tag, write_gzip_compound_tag},
    tag::NbtTag,
};
use verdantgolem_util::math::vector2::Vector2;
use verdantgolem_world::world_info::MAXIMUM_SUPPORTED_WORLD_DATA_VERSION;

type ChunkSet = FxHashSet<Vector2<i32>>;

pub(super) struct ForcedChunkDelta {
    pub added: Vec<Vector2<i32>>,
    pub removed: Vec<Vector2<i32>>,
}

/// Mutations are private to the world API: changing the list without updating
/// its saved data and loading tickets is not supported.
pub(super) struct ForcedChunkStore {
    path: PathBuf,
    chunks: ChunkSet,
    root: NbtCompound,
    read_error: Option<(io::ErrorKind, String)>,
    tickets_initialized: bool,
}

impl ForcedChunkStore {
    pub fn load(dimension_folder: &Path) -> Self {
        let canonical = dimension_folder.join("data/minecraft/chunks.dat");
        let legacy = dimension_folder.join("data/chunks.dat");
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", MAXIMUM_SUPPORTED_WORLD_DATA_VERSION);
        let mut store = Self {
            path: canonical.clone(),
            chunks: ChunkSet::default(),
            root,
            read_error: None,
            tickets_initialized: false,
        };

        // `exists()` would hide permission errors and dangling symlinks, then
        // permit a later command to overwrite a saved list we never read.
        let selected = path_exists(&canonical).and_then(|exists| {
            if exists {
                Ok((canonical, true))
            } else {
                path_exists(&legacy).map(|exists| {
                    if exists {
                        (legacy, true)
                    } else {
                        (canonical, false)
                    }
                })
            }
        });
        let result = match selected {
            Ok((path, exists)) => {
                store.path = path;
                if exists {
                    store.read_existing()
                } else {
                    Ok(())
                }
            }
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            store.read_error = Some((error.kind(), error.to_string()));
        }
        store
    }

    fn read_existing(&mut self) -> io::Result<()> {
        let file = File::open(&self.path)
            .map_err(|error| path_error(&self.path, "read forced chunks", &error))?;
        let root = read_gzip_compound_tag(file).map_err(|error| {
            path_error(&self.path, "decode forced chunks", &io::Error::other(error))
        })?;
        let positions = root
            .get_compound("data")
            .and_then(|data| data.get_long_array("Forced"))
            .ok_or_else(|| {
                path_error(
                    &self.path,
                    "decode forced chunks",
                    &io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected data.Forced long array",
                    ),
                )
            })?;
        let chunks: ChunkSet = positions.iter().map(|position| unpack(*position)).collect();
        if let Some(position) = chunks.iter().find(|position| !valid_position(**position)) {
            return Err(path_error(
                &self.path,
                "decode forced chunks",
                &io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "chunk [{}, {}] is outside the world",
                        position.x, position.y
                    ),
                ),
            ));
        }
        self.chunks = chunks;
        self.root = root;
        Ok(())
    }

    pub fn snapshot(&self) -> io::Result<&ChunkSet> {
        self.check_readable()?;
        Ok(&self.chunks)
    }

    fn check_readable(&self) -> io::Result<()> {
        if let Some((kind, error)) = &self.read_error {
            return Err(io::Error::new(
                *kind,
                format!("{error}; saved data retained, repair it and reload the world"),
            ));
        }
        Ok(())
    }

    /// Call only after the level's world portal/listener is attached. Repeated
    /// calls do not duplicate tickets. Changes made before this call remain
    /// durable but are dispatched together with the restored list, not early.
    pub fn initialize_tickets(
        &mut self,
        apply: impl FnOnce(&ForcedChunkDelta),
    ) -> io::Result<usize> {
        self.check_readable()?;
        if self.tickets_initialized {
            return Ok(0);
        }
        let added = sorted_positions(&self.chunks);
        let count = added.len();
        if count > 0 {
            apply(&ForcedChunkDelta {
                added,
                removed: Vec::new(),
            });
        }
        self.tickets_initialized = true;
        Ok(count)
    }

    pub fn add(
        &mut self,
        positions: impl IntoIterator<Item = Vector2<i32>>,
        apply: impl FnOnce(&ForcedChunkDelta),
    ) -> io::Result<usize> {
        self.check_readable()?;
        let mut next = self.chunks.clone();
        for position in positions {
            if !valid_position(position) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "chunk [{}, {}] is outside the world",
                        position.x, position.y
                    ),
                ));
            }
            next.insert(position);
            if next.len() > MAX_ARRAY_LENGTH {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("forced chunk saved-data limit is {MAX_ARRAY_LENGTH}"),
                ));
            }
        }
        self.commit(next, apply, write_atomic)
    }

    pub fn remove(
        &mut self,
        positions: impl IntoIterator<Item = Vector2<i32>>,
        apply: impl FnOnce(&ForcedChunkDelta),
    ) -> io::Result<usize> {
        self.check_readable()?;
        let mut next = self.chunks.clone();
        for position in positions {
            next.remove(&position);
        }
        self.commit(next, apply, write_atomic)
    }

    pub fn clear(&mut self, apply: impl FnOnce(&ForcedChunkDelta)) -> io::Result<usize> {
        self.check_readable()?;
        self.commit(ChunkSet::default(), apply, write_atomic)
    }

    fn commit(
        &mut self,
        next: ChunkSet,
        apply: impl FnOnce(&ForcedChunkDelta),
        write: impl FnOnce(&Path, &NbtCompound) -> Result<(), AtomicWriteError>,
    ) -> io::Result<usize> {
        if next == self.chunks {
            return Ok(0);
        }
        // Match the NBT reader's bound: never acknowledge a file that this
        // server would reject at the next restart, even with a large rule limit.
        if next.len() > MAX_ARRAY_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("forced chunk saved-data limit is {MAX_ARRAY_LENGTH}"),
            ));
        }
        let delta = ForcedChunkDelta {
            added: sorted_positions(&next.difference(&self.chunks).copied().collect()),
            removed: sorted_positions(&self.chunks.difference(&next).copied().collect()),
        };
        let count = delta.added.len() + delta.removed.len();
        let mut root = self.root.clone();
        let mut data = root.get_compound("data").cloned().unwrap_or_default();
        data.put(
            "Forced",
            NbtTag::LongArray(sorted_positions(&next).into_iter().map(pack).collect()),
        );
        root.put_compound("data", data);

        let durability_error = match write(&self.path, &root) {
            Ok(()) => None,
            Err(AtomicWriteError::BeforeCommit(error)) => return Err(error),
            #[cfg(any(unix, test))]
            Err(AtomicWriteError::AfterCommit(error)) => Some(error),
        };
        // The atomic replacement is the commit point. Keep runtime state and
        // tickets in agreement with it, even if the subsequent directory sync
        // failed. Such a command still returns an explicit durability error.
        self.chunks = next;
        self.root = root;
        if self.tickets_initialized {
            apply(&delta);
        }
        durability_error.map_or(Ok(count), Err)
    }
}

fn sorted_positions(chunks: &ChunkSet) -> Vec<Vector2<i32>> {
    let mut positions: Vec<_> = chunks.iter().copied().collect();
    positions.sort_unstable_by_key(|position| (position.x, position.y));
    positions
}

/// Match World's horizontal block bounds before passing coordinates to the
/// ticket propagation code, which adds neighbouring chunk offsets.
fn valid_position(position: Vector2<i32>) -> bool {
    const CHUNK_BOUNDS: std::ops::Range<i32> = (-30_000_000 >> 4)..(30_000_000 >> 4);
    CHUNK_BOUNDS.contains(&position.x) && CHUNK_BOUNDS.contains(&position.y)
}

/// Vanilla ChunkPos.asLong: signed X in the low 32 bits and signed Z above it.
fn pack(position: Vector2<i32>) -> i64 {
    (u64::from(position.x as u32) | (u64::from(position.y as u32) << 32)) as i64
}

const fn unpack(position: i64) -> Vector2<i32> {
    Vector2::new(position as i32, (position >> 32) as i32)
}

fn path_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(path_error(path, "inspect forced chunks", &error)),
    }
}

fn path_error(path: &Path, operation: &str, error: &io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{operation} {}: {error}", path.display()),
    )
}

enum AtomicWriteError {
    BeforeCommit(io::Error),
    // Only Unix syncs the directory after replacement; tests can inject the
    // post-commit failure on any platform to exercise state/ticket consistency.
    #[cfg(any(unix, test))]
    AfterCommit(io::Error),
}

struct PendingFile(PathBuf);

impl Drop for PendingFile {
    fn drop(&mut self) {
        // Also clean up after serialization, sync, or rename failures. Once a
        // rename succeeded this path no longer exists.
        let _ = fs::remove_file(&self.0);
    }
}

fn write_atomic(path: &Path, root: &NbtCompound) -> Result<(), AtomicWriteError> {
    let before =
        |error| AtomicWriteError::BeforeCommit(path_error(path, "save forced chunks", &error));
    let parent = path.parent().ok_or_else(|| {
        before(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing data directory",
        ))
    })?;
    fs::create_dir_all(parent).map_err(before)?;
    // Open before replacing anything, so errors acquiring the sync handle are
    // pre-commit failures. Windows does not expose directory fsync via std.
    #[cfg(unix)]
    let directory = File::open(parent).map_err(before)?;
    let temporary = parent.join(format!(".chunks-{}.dat.tmp", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(before)?;
    let pending = PendingFile(temporary);
    write_gzip_compound_tag(root.clone(), &mut file)
        .map_err(|error| before(io::Error::other(error)))?;
    file.sync_all().map_err(before)?;
    drop(file);
    fs::rename(&pending.0, path).map_err(before)?;
    #[cfg(unix)]
    directory.sync_all().map_err(|error| {
        AtomicWriteError::AfterCommit(path_error(
            path,
            "forced chunks replaced, but directory sync failed (durability unconfirmed)",
            &error,
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn positions(values: &[(i32, i32)]) -> Vec<Vector2<i32>> {
        values.iter().map(|&(x, z)| Vector2::new(x, z)).collect()
    }

    fn saved_root(values: &[(i32, i32)]) -> NbtCompound {
        let mut data = NbtCompound::new();
        data.put(
            "Forced",
            NbtTag::LongArray(positions(values).into_iter().map(pack).collect()),
        );
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", MAXIMUM_SUPPORTED_WORLD_DATA_VERSION);
        root.put_compound("data", data);
        root
    }

    fn write_fixture(path: &Path, root: NbtCompound) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_gzip_compound_tag(root, File::create(path).unwrap()).unwrap();
    }

    fn apply_to(live: &mut ChunkSet, delta: &ForcedChunkDelta) {
        for position in &delta.removed {
            assert!(live.remove(position), "ticket removed more than once");
        }
        for position in &delta.added {
            assert!(live.insert(*position), "ticket installed more than once");
        }
    }

    #[test]
    fn vanilla_packing_round_trips_signed_coordinates() {
        assert_eq!(pack(Vector2::new(-1, 0)), 0xFFFF_FFFF);
        assert_eq!(pack(Vector2::new(0, -1)), -4_294_967_296);
        for x in [i32::MIN, -1, 0, 1, i32::MAX] {
            for z in [i32::MIN, -1, 0, 1, i32::MAX] {
                let position = Vector2::new(x, z);
                assert_eq!(unpack(pack(position)), position);
            }
        }
    }

    #[test]
    fn invalid_positions_do_not_change_saved_data_or_tickets() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        store.initialize_tickets(|_| {}).unwrap();
        let boundary_positions = [Vector2::new(-1_875_000, 1_874_999)];
        store.add(boundary_positions, |_| {}).unwrap();
        let before = fs::read(&store.path).unwrap();
        for position in [
            Vector2::new(1_875_000, 0),
            Vector2::new(0, -1_875_001),
            Vector2::new(i32::MAX, i32::MIN),
        ] {
            assert!(
                store
                    .add([Vector2::new(0, 0), position], |_| panic!("invalid ticket"))
                    .is_err()
            );
            assert_eq!(store.snapshot().unwrap().len(), 1);
            assert_eq!(fs::read(&store.path).unwrap(), before);
        }
        assert_eq!(
            ForcedChunkStore::load(dir.path()).snapshot().unwrap(),
            &boundary_positions.into_iter().collect::<ChunkSet>()
        );
    }

    #[test]
    fn out_of_bounds_saved_positions_are_preserved_without_dispatch() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data/minecraft/chunks.dat");
        write_fixture(&path, saved_root(&[(i32::MAX, 0)]));
        let before = fs::read(&path).unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        assert!(store.snapshot().is_err());
        assert!(
            store
                .initialize_tickets(|_| panic!("invalid restore"))
                .is_err()
        );
        assert!(store.clear(|_| panic!("invalid clear")).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn missing_store_stays_empty_without_creating_files() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        assert!(store.snapshot().unwrap().is_empty());
        assert_eq!(store.clear(|_| panic!("unexpected delta")).unwrap(), 0);
        assert!(!store.path.exists());
    }

    #[test]
    fn round_trip_restores_tickets_once_after_initialization() {
        let dir = TempDir::new().unwrap();
        let expected = positions(&[(-24, 7), (8, -30), (0, 0)]);
        let mut store = ForcedChunkStore::load(dir.path());
        assert_eq!(
            store
                .add(expected.clone(), |_| panic!("early dispatch"))
                .unwrap(),
            3
        );
        let raw = fs::read(&store.path).unwrap();
        assert_eq!(&raw[..2], &[0x1F, 0x8B]);
        let root = read_gzip_compound_tag(File::open(&store.path).unwrap()).unwrap();
        assert_eq!(
            root.get_compound("data")
                .unwrap()
                .get_long_array("Forced")
                .unwrap()
                .len(),
            3
        );

        let mut restarted = ForcedChunkStore::load(dir.path());
        let mut live = ChunkSet::default();
        assert_eq!(
            restarted
                .initialize_tickets(|delta| apply_to(&mut live, delta))
                .unwrap(),
            3
        );
        assert_eq!(
            restarted
                .initialize_tickets(|_| panic!("duplicate restore"))
                .unwrap(),
            0
        );
        assert_eq!(live, expected.into_iter().collect::<ChunkSet>());
        assert_eq!(&live, restarted.snapshot().unwrap());
    }

    #[test]
    fn duplicate_add_and_absent_remove_do_not_write_or_dispatch() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        store.initialize_tickets(|_| {}).unwrap();
        store.add(positions(&[(2, 3), (2, 3)]), |_| {}).unwrap();
        let before = fs::read(&store.path).unwrap();
        assert_eq!(
            store
                .add(positions(&[(2, 3)]), |_| panic!("duplicate"))
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .remove(positions(&[(9, 9)]), |_| panic!("absent"))
                .unwrap(),
            0
        );
        assert_eq!(fs::read(&store.path).unwrap(), before);
    }

    #[test]
    fn remove_and_remove_all_are_persisted_before_ticket_changes() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        let mut live = ChunkSet::default();
        store
            .initialize_tickets(|delta| apply_to(&mut live, delta))
            .unwrap();
        store
            .add(positions(&[(1, 1), (2, 2), (3, 3)]), |delta| {
                apply_to(&mut live, delta);
            })
            .unwrap();
        assert_eq!(
            store
                .remove(positions(&[(1, 1), (8, 8), (1, 1)]), |delta| {
                    let restarted = ForcedChunkStore::load(dir.path());
                    assert!(!restarted.snapshot().unwrap().contains(&Vector2::new(1, 1)));
                    apply_to(&mut live, delta);
                })
                .unwrap(),
            1
        );
        assert_eq!(
            live,
            *ForcedChunkStore::load(dir.path()).snapshot().unwrap()
        );
        assert_eq!(
            store
                .clear(|delta| {
                    assert!(
                        ForcedChunkStore::load(dir.path())
                            .snapshot()
                            .unwrap()
                            .is_empty()
                    );
                    apply_to(&mut live, delta);
                })
                .unwrap(),
            2
        );
        assert!(live.is_empty());
        assert_eq!(store.clear(|_| panic!("duplicate clear")).unwrap(), 0);
        assert!(
            store.path.is_file(),
            "persist an empty list rather than exposing a legacy fallback"
        );
    }

    #[test]
    fn dimensions_use_independent_saved_lists() {
        let dir = TempDir::new().unwrap();
        let dimensions = [
            "dimensions/minecraft/overworld",
            "dimensions/minecraft/the_nether",
            "dimensions/minecraft/the_end",
        ];
        for (index, name) in dimensions.iter().enumerate() {
            let mut store = ForcedChunkStore::load(&dir.path().join(name));
            store.add([Vector2::new(index as i32, -1)], |_| {}).unwrap();
        }
        for (index, name) in dimensions.iter().enumerate() {
            let store = ForcedChunkStore::load(&dir.path().join(name));
            assert_eq!(
                store.snapshot().unwrap(),
                &std::iter::once(Vector2::new(index as i32, -1)).collect::<ChunkSet>()
            );
        }
        assert!(!dir.path().join("data").exists());
    }

    #[test]
    fn legacy_path_is_read_and_updated_without_changing_dimensions() {
        let dir = TempDir::new().unwrap();
        let dimension = dir.path().join("DIM-1");
        let legacy = dimension.join("data/chunks.dat");
        write_fixture(&legacy, saved_root(&[(5, -4)]));
        let mut store = ForcedChunkStore::load(&dimension);
        assert_eq!(store.path, legacy);
        assert_eq!(store.clear(|_| {}).unwrap(), 1);
        assert!(
            ForcedChunkStore::load(&dimension)
                .snapshot()
                .unwrap()
                .is_empty()
        );
        assert!(!dimension.join("data/minecraft/chunks.dat").exists());
        assert!(
            ForcedChunkStore::load(dir.path())
                .snapshot()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn canonical_empty_data_shadows_legacy_data() {
        let dir = TempDir::new().unwrap();
        write_fixture(&dir.path().join("data/chunks.dat"), saved_root(&[(1, 2)]));
        write_fixture(
            &dir.path().join("data/minecraft/chunks.dat"),
            saved_root(&[]),
        );
        let store = ForcedChunkStore::load(dir.path());
        assert!(store.snapshot().unwrap().is_empty());
        assert_eq!(store.path, dir.path().join("data/minecraft/chunks.dat"));
    }

    #[test]
    fn malformed_data_is_preserved_and_all_operations_report_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data/minecraft/chunks.dat");
        write_fixture(&dir.path().join("data/chunks.dat"), saved_root(&[(1, 2)]));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not gzip NBT").unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        assert!(store.snapshot().is_err());
        assert!(store.initialize_tickets(|_| panic!("bad restore")).is_err());
        assert!(
            store
                .add(positions(&[(2, 3)]), |_| panic!("bad add"))
                .is_err()
        );
        assert!(
            store
                .remove(positions(&[(2, 3)]), |_| panic!("bad remove"))
                .is_err()
        );
        assert!(store.clear(|_| panic!("bad clear")).is_err());
        assert_eq!(fs::read(path).unwrap(), b"not gzip NBT");
    }

    #[test]
    fn missing_or_wrong_forced_tag_is_not_treated_as_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data/minecraft/chunks.dat");
        for root in [NbtCompound::new(), {
            let mut root = NbtCompound::new();
            let mut data = NbtCompound::new();
            data.put_list("Forced", Vec::new());
            root.put_compound("data", data);
            root
        }] {
            write_fixture(&path, root);
            let before = fs::read(&path).unwrap();
            let mut store = ForcedChunkStore::load(dir.path());
            assert!(store.clear(|_| {}).is_err());
            assert_eq!(fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn unreadable_saved_file_is_not_replaced() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data/minecraft/chunks.dat");
        fs::create_dir_all(&path).unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        assert!(store.snapshot().is_err());
        assert!(
            store
                .add(positions(&[(2, 3)]), |_| panic!("bad delta"))
                .is_err()
        );
        assert!(path.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_canonical_symlink_does_not_fall_back_or_get_overwritten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data/minecraft/chunks.dat");
        write_fixture(&dir.path().join("data/chunks.dat"), saved_root(&[(1, 2)]));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(dir.path().join("missing.dat"), &path).unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        assert!(store.clear(|_| {}).is_err());
        assert!(fs::symlink_metadata(path).unwrap().is_symlink());
    }

    #[test]
    fn unknown_saved_fields_survive_mutations() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data/minecraft/chunks.dat");
        let mut root = saved_root(&[(1, 2)]);
        root.put_string("extension", "preserve root".to_string());
        let mut data = root.get_compound("data").unwrap().clone();
        data.put_string("extension", "preserve data".to_string());
        root.put_compound("data", data);
        write_fixture(&path, root);
        let mut store = ForcedChunkStore::load(dir.path());
        store.add(positions(&[(3, 4)]), |_| {}).unwrap();
        let root = read_gzip_compound_tag(File::open(path).unwrap()).unwrap();
        assert_eq!(root.get_string("extension"), Some("preserve root"));
        assert_eq!(
            root.get_compound("data").unwrap().get_string("extension"),
            Some("preserve data")
        );
    }

    #[test]
    fn write_failures_preserve_runtime_tickets_and_previous_file() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        let mut live = ChunkSet::default();
        store
            .initialize_tickets(|delta| apply_to(&mut live, delta))
            .unwrap();
        store
            .add(positions(&[(3, 4)]), |delta| apply_to(&mut live, delta))
            .unwrap();
        let before = fs::read(&store.path).unwrap();
        let parent = store.path.parent().unwrap().to_path_buf();
        let backup = dir.path().join("saved-data");
        fs::rename(&parent, &backup).unwrap();
        fs::write(&parent, b"block data directory creation").unwrap();
        assert!(
            store
                .add(positions(&[(5, 6)]), |_| panic!("failed add dispatched"))
                .is_err()
        );
        assert!(
            store
                .remove(positions(&[(3, 4)]), |_| panic!("failed remove dispatched"))
                .is_err()
        );
        assert!(store.clear(|_| panic!("failed clear dispatched")).is_err());
        assert_eq!(store.snapshot().unwrap(), &live);
        assert_eq!(fs::read(backup.join("chunks.dat")).unwrap(), before);
        fs::remove_file(&parent).unwrap();
        fs::rename(&backup, &parent).unwrap();
        assert_eq!(
            ForcedChunkStore::load(dir.path()).snapshot().unwrap(),
            &live
        );
        assert_eq!(store.clear(|delta| apply_to(&mut live, delta)).unwrap(), 1);
    }

    #[test]
    fn failed_atomic_rename_cleans_up_temporary_file() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        fs::create_dir_all(&store.path).unwrap();
        assert!(
            store
                .add(positions(&[(1, 2)]), |_| panic!("failed delta"))
                .is_err()
        );
        assert!(store.snapshot().unwrap().is_empty());
        let entries: Vec<_> = fs::read_dir(store.path.parent().unwrap())
            .unwrap()
            .collect();
        assert_eq!(entries.len(), 1);
        assert!(store.path.is_dir());
    }

    #[test]
    fn post_commit_sync_error_reports_failure_but_keeps_disk_and_tickets_consistent() {
        let dir = TempDir::new().unwrap();
        let mut store = ForcedChunkStore::load(dir.path());
        store.initialize_tickets(|_| {}).unwrap();
        let next: ChunkSet = positions(&[(9, 4)]).into_iter().collect();
        let mut live = ChunkSet::default();
        let result = store.commit(
            next.clone(),
            |delta| apply_to(&mut live, delta),
            |path, root| {
                write_atomic(path, root)?;
                Err(AtomicWriteError::AfterCommit(io::Error::other(
                    "injected directory sync failure",
                )))
            },
        );
        assert!(result.is_err());
        assert_eq!(live, next);
        assert_eq!(store.snapshot().unwrap(), &next);
        assert_eq!(
            ForcedChunkStore::load(dir.path()).snapshot().unwrap(),
            &next
        );
    }

    #[test]
    fn concurrent_mutations_serialize_file_and_ticket_deltas_under_the_world_lock() {
        let dir = TempDir::new().unwrap();
        let store = Arc::new(Mutex::new(ForcedChunkStore::load(dir.path())));
        let live = Arc::new(Mutex::new(ChunkSet::default()));
        store.lock().unwrap().initialize_tickets(|_| {}).unwrap();
        let handles: Vec<_> = (0..4)
            .map(|x| {
                let store = store.clone();
                let live = live.clone();
                std::thread::spawn(move || {
                    store
                        .lock()
                        .unwrap()
                        .add(positions(&[(x, 1), (x, 2)]), |delta| {
                            apply_to(&mut live.lock().unwrap(), delta);
                        })
                        .unwrap();
                    store
                        .lock()
                        .unwrap()
                        .remove(positions(&[(x, 1)]), |delta| {
                            apply_to(&mut live.lock().unwrap(), delta);
                        })
                        .unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let store = store.lock().unwrap();
        let live = live.lock().unwrap();
        assert_eq!(live.len(), 4);
        assert_eq!(store.snapshot().unwrap(), &*live);
        assert_eq!(
            ForcedChunkStore::load(dir.path()).snapshot().unwrap(),
            &*live
        );
    }
}
