//! Deterministic action scheduling. Delays are simulation ticks, never wall time.

use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionKind {
    Attack,
    Use,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionMode {
    Once,
    Continuous,
    Interval(NonZeroU32),
}

impl ActionMode {
    pub fn interval(ticks: u32) -> Result<Self, String> {
        NonZeroU32::new(ticks)
            .map(Self::Interval)
            .ok_or_else(|| "Action interval must be a positive number of ticks".to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementDirection {
    Forward,
    Backward,
    Left,
    Right,
}

impl MovementDirection {
    /// Local-space input: +x is left and +z is forward, rotated by entity yaw.
    #[must_use]
    pub const fn input(self) -> (f64, f64) {
        match self {
            Self::Forward => (0.0, 1.0),
            Self::Backward => (0.0, -1.0),
            Self::Left => (1.0, 0.0),
            Self::Right => (-1.0, 0.0),
        }
    }
}

#[derive(Default)]
struct ScheduledAction {
    mode: Option<ActionMode>,
    ticks_until_action: u32,
}

impl ScheduledAction {
    const fn set(&mut self, mode: Option<ActionMode>) {
        self.mode = mode;
        self.ticks_until_action = 0;
    }

    const fn tick(&mut self, continuous_interval: u32) -> bool {
        let Some(mode) = self.mode else {
            return false;
        };
        if self.ticks_until_action > 0 {
            self.ticks_until_action -= 1;
            return false;
        }
        match mode {
            ActionMode::Once => self.mode = None,
            ActionMode::Continuous => self.ticks_until_action = continuous_interval - 1,
            ActionMode::Interval(ticks) => self.ticks_until_action = ticks.get() - 1,
        }
        true
    }
}

#[derive(Default)]
pub(super) struct ActionScheduler {
    attack: ScheduledAction,
    use_item: ScheduledAction,
    pub movement: Option<MovementDirection>,
    release_use_next_tick: bool,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct TickActions {
    pub attack: bool,
    pub use_item: bool,
    pub release_use: bool,
    pub movement: Option<MovementDirection>,
}

impl ActionScheduler {
    pub const fn is_active(&self, kind: ActionKind) -> bool {
        match kind {
            ActionKind::Attack => self.attack.mode.is_some(),
            ActionKind::Use => self.use_item.mode.is_some(),
        }
    }

    pub const fn set(&mut self, kind: ActionKind, mode: Option<ActionMode>) {
        match kind {
            ActionKind::Attack => self.attack.set(mode),
            ActionKind::Use => {
                self.use_item.set(mode);
                self.release_use_next_tick = false;
            }
        }
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    /// A pulse (once/interval) releases on the following tick. Continuous use
    /// holds a charged item, retrying instant uses every four vanilla ticks.
    /// Call only on game ticks: `/tick freeze` therefore pauses exact delays.
    pub fn tick(&mut self) -> TickActions {
        let release_use = std::mem::take(&mut self.release_use_next_tick);
        let continuous_use = self.use_item.mode == Some(ActionMode::Continuous);
        let use_item = self.use_item.tick(4);
        self.release_use_next_tick = use_item && !continuous_use;
        TickActions {
            attack: self.attack.tick(1),
            use_item,
            release_use,
            movement: self.movement,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_runs_on_next_tick_and_does_not_repeat() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::Once));
        assert!(actions.tick().attack);
        assert!(!actions.is_active(ActionKind::Attack));
        for _ in 0..10 {
            assert!(!actions.tick().attack);
        }
    }

    #[test]
    fn interval_has_no_extra_off_by_one_tick() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::interval(3).unwrap()));
        let actual: Vec<_> = (0..10).filter(|_| actions.tick().attack).collect();
        assert_eq!(actual, vec![0, 3, 6, 9]);
    }

    #[test]
    fn positive_interval_boundaries() {
        assert!(ActionMode::interval(0).is_err());
        assert!(ActionMode::interval(u32::MAX).is_ok());
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::interval(1).unwrap()));
        for _ in 0..5 {
            assert!(actions.tick().attack);
        }
    }

    #[test]
    fn replacing_action_resets_delay() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::interval(100).unwrap()));
        assert!(actions.tick().attack);
        assert!(!actions.tick().attack);
        actions.set(ActionKind::Attack, Some(ActionMode::Once));
        assert!(actions.tick().attack);
        assert!(!actions.tick().attack);
    }

    #[test]
    fn attack_and_use_schedules_are_independent() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::Continuous));
        actions.set(ActionKind::Use, Some(ActionMode::interval(3).unwrap()));
        for tick in 0..10 {
            let plan = actions.tick();
            assert!(plan.attack);
            assert_eq!(plan.use_item, tick % 3 == 0);
            assert_eq!(plan.release_use, tick % 3 == 1);
        }
    }

    #[test]
    fn continuous_use_holds_instead_of_releasing_between_retries() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Use, Some(ActionMode::Continuous));
        for tick in 0..12 {
            let plan = actions.tick();
            assert_eq!(plan.use_item, tick % 4 == 0);
            assert!(!plan.release_use);
        }
    }

    #[test]
    fn once_and_one_tick_intervals_release_pressed_items() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Use, Some(ActionMode::Once));
        assert!(actions.tick().use_item);
        let release = actions.tick();
        assert!(release.release_use);
        assert!(!release.use_item);
        assert!(!actions.tick().release_use);
        actions.set(ActionKind::Use, Some(ActionMode::interval(1).unwrap()));
        assert!(actions.tick().use_item);
        let again = actions.tick();
        assert!(again.release_use && again.use_item);
    }

    #[test]
    fn stop_clears_every_action_pending_release_and_movement() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::Continuous));
        actions.set(ActionKind::Use, Some(ActionMode::Once));
        actions.movement = Some(MovementDirection::Forward);
        actions.tick();
        actions.stop();
        for _ in 0..6 {
            assert_eq!(actions.tick(), TickActions::default());
        }
    }

    #[test]
    fn no_game_ticks_means_no_wall_clock_catch_up() {
        let mut actions = ActionScheduler::default();
        actions.set(ActionKind::Attack, Some(ActionMode::interval(3).unwrap()));
        assert!(actions.tick().attack);
        // Frozen/network-only ticks do not invoke this scheduler.
        assert!(!actions.tick().attack);
        assert!(!actions.tick().attack);
        assert!(actions.tick().attack);
    }

    #[test]
    fn movement_directions_are_opposites_and_unit_length() {
        for (a, b) in [
            (MovementDirection::Forward, MovementDirection::Backward),
            (MovementDirection::Left, MovementDirection::Right),
        ] {
            let (x, z) = a.input();
            let (opposite_x, opposite_z) = b.input();
            assert_eq!((x + opposite_x, z + opposite_z), (0.0, 0.0));
            assert_eq!(x * x + z * z, 1.0);
        }
    }
}
