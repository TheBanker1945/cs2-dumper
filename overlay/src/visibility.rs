use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::game::{GameState, PlayerData, HEAD_BONE};
use crate::smoke;

/// How long the spotted flag keeps counting after it drops. The server's line-of-sight
/// check flickers off for a second or two even with a target standing in plain view.
const SPOTTED_MEMORY: Duration = Duration::from_millis(1000);

/// Whether the local player can see a target: under the crosshair (the game's eye
/// trace, which walls stop) or recently spotted (the server's line-of-sight check that
/// drives the radar), and in both cases with no smoke between the eyes and its head.
pub struct Visibility {
    /// Pawn index -> when the local player last spotted it.
    spotted_at: HashMap<u32, Instant>,
}

impl Visibility {
    pub fn new() -> Self {
        Self {
            spotted_at: HashMap::new(),
        }
    }

    /// Call once per frame, before any `can_see`.
    pub fn update(&mut self, state: &GameState, now: Instant) {
        if let Some(local) = state.local() {
            for player in state.players.iter().filter(|p| spotted_by(p, local)) {
                self.spotted_at.insert(player.pawn_index, now);
            }
        }
        self.spotted_at
            .retain(|_, at| now.duration_since(*at) <= SPOTTED_MEMORY);
    }

    pub fn can_see(&self, state: &GameState, target: &PlayerData) -> bool {
        let Some(view) = &state.local_view else {
            return false;
        };
        let in_sight = state.crosshair_entity == Some(target.pawn_index)
            || self.spotted_at.contains_key(&target.pawn_index);
        in_sight && !smoke::blocks_view(view.eye, target.bones[HEAD_BONE], &state.smokes)
    }
}

/// The spotted mask has bit `index - 1` set for each player who can see the target.
fn spotted_by(target: &PlayerData, viewer: &PlayerData) -> bool {
    match viewer.index {
        1..=64 => target.spotted_by >> (viewer.index - 1) & 1 != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{LocalView, MAX_BONES};
    use crate::math::Vec3;

    const LOCAL: u32 = 1;

    fn player(pawn_index: u32, index: u32, spotted_by: u64) -> PlayerData {
        let mut bones = [Vec3::default(); MAX_BONES];
        bones[HEAD_BONE] = Vec3 { x: 1000.0, y: 0.0, z: 64.0 };
        PlayerData {
            name: String::new(),
            team: 2,
            health: 100,
            bones,
            is_local: index == LOCAL,
            weapon_id: 7,
            ammo: 30,
            index,
            pawn_index,
            is_bot: true,
            spotted_by,
        }
    }

    fn state(target: PlayerData, crosshair: Option<u32>, smokes: Vec<Vec3>) -> GameState {
        GameState {
            players: vec![player(100, LOCAL, 0), target],
            view_matrix: Default::default(),
            screen_width: 0.0,
            screen_height: 0.0,
            crosshair_entity: crosshair,
            local_view: Some(LocalView {
                eye: Vec3 { x: 0.0, y: 0.0, z: 64.0 },
                pitch: 0.0,
                yaw: 0.0,
                sensitivity: 1.0,
            }),
            smokes,
        }
    }

    fn sees(state: &GameState) -> bool {
        let mut v = Visibility::new();
        v.update(state, Instant::now());
        v.can_see(state, &state.players[1])
    }

    #[test]
    fn under_the_crosshair_is_visible_without_the_spotted_flag() {
        assert!(sees(&state(player(200, 2, 0), Some(200), vec![])));
    }

    #[test]
    fn spotted_by_us_is_visible() {
        assert!(sees(&state(player(200, 2, 1 << (LOCAL - 1)), None, vec![])));
    }

    #[test]
    fn neither_spotted_nor_under_the_crosshair_is_hidden() {
        assert!(!sees(&state(player(200, 2, 0), None, vec![])));
    }

    #[test]
    fn only_our_own_spotted_bit_counts() {
        assert!(!sees(&state(player(200, 2, 1 << 4), None, vec![])));
    }

    #[test]
    fn smoke_hides_even_a_target_under_the_crosshair() {
        let smoke = Vec3 { x: 500.0, y: 0.0, z: 32.0 };
        assert!(!sees(&state(player(200, 2, 1), Some(200), vec![smoke])));
    }

    #[test]
    fn spotted_flag_is_remembered_briefly() {
        let start = Instant::now();
        let mut v = Visibility::new();
        v.update(&state(player(200, 2, 1), None, vec![]), start);

        let dropped = state(player(200, 2, 0), None, vec![]);
        v.update(&dropped, start + Duration::from_millis(900));
        assert!(v.can_see(&dropped, &dropped.players[1]));

        v.update(&dropped, start + Duration::from_millis(1100));
        assert!(!v.can_see(&dropped, &dropped.players[1]));
    }
}
