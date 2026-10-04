use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::game::{GameState, PlayerData};
use crate::visibility::Visibility;
use crate::weapons::is_gun;

/// How long a target must stay under the crosshair before the first shot.
const REACTION_DELAY: Duration = Duration::from_millis(30);

/// Fires at enemies under the crosshair with a clear line of sight.
pub struct TriggerBot {
    on_target_since: Option<Instant>,
    /// The left button is currently held down by us (not by the user).
    pressed: bool,
}

impl TriggerBot {
    pub fn new() -> Self {
        Self {
            on_target_since: None,
            pressed: false,
        }
    }

    /// Call once per frame. `armed` is false while the feature is off, the hold key
    /// is up, the aim isn't on the head yet, the menu is open or CS2 isn't the
    /// foreground window.
    pub fn update(&mut self, armed: bool, state: &GameState, visibility: &Visibility) {
        if !(armed && clear_shot_at_enemy_bot(state, visibility)) {
            self.on_target_since = None;
            self.release();
            return;
        }

        let since = *self.on_target_since.get_or_insert_with(Instant::now);
        // Tap rather than hold: one frame down, one frame up. Semi-automatic guns need
        // a fresh press per shot, and automatics still fire close to their full rate.
        if self.pressed {
            self.release();
        } else if since.elapsed() >= REACTION_DELAY && !user_holding_fire() {
            send_left(MOUSEEVENTF_LEFTDOWN);
            self.pressed = true;
        }
    }

    fn release(&mut self) {
        if self.pressed {
            send_left(MOUSEEVENTF_LEFTUP);
            self.pressed = false;
        }
    }
}

impl Drop for TriggerBot {
    fn drop(&mut self) {
        self.release();
    }
}

fn clear_shot_at_enemy_bot(state: &GameState, visibility: &Visibility) -> bool {
    let (Some(local), Some(target)) = (state.local(), state.crosshair_entity) else {
        return false;
    };
    // Guns only: left click with a knife, grenade or the C4 would slash, throw or plant.
    is_gun(local.weapon_id)
        && enemy_player(state, local, target).is_some_and(|enemy| visibility.can_see(state, enemy))
}

/// The player driving `pawn`, provided every controller pointing at that pawn is an
/// enemy.
pub fn enemy_player<'a>(
    state: &'a GameState,
    local: &PlayerData,
    pawn: u32,
) -> Option<&'a PlayerData> {
    let mut drivers = state.players.iter().filter(|p| p.pawn_index == pawn).peekable();
    let first = *drivers.peek()?;
    drivers
        .all(|p| p.team != local.team)
        .then_some(first)
}

/// Only meaningful while we aren't pressing: GetAsyncKeyState also sees injected input.
fn user_holding_fire() -> bool {
    unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0 }
}

fn send_left(flags: MOUSE_EVENT_FLAGS) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{LocalView, HEAD_BONE, MAX_BONES};
    use crate::math::Vec3;

    const AK47: u16 = 7;
    const KNIFE: u16 = 42;

    /// Players get controller index `pawn_index / 100`, with their head 1000 units
    /// down the x axis. Nobody is spotted: the crosshair alone shows line of sight.
    fn player(pawn_index: u32, team: u8, is_bot: bool, is_local: bool) -> PlayerData {
        let mut bones = [Vec3::default(); MAX_BONES];
        bones[HEAD_BONE] = Vec3 { x: 1000.0, y: 0.0, z: 64.0 };
        PlayerData {
            name: String::new(),
            team,
            health: 100,
            bones,
            is_local,
            weapon_id: AK47,
            ammo: 30,
            index: pawn_index / 100,
            pawn_index,
            is_bot,
            spotted_by: 0,
        }
    }

    fn shot_with_smoke(target: u32, players: Vec<PlayerData>, smokes: Vec<Vec3>) -> bool {
        let state = GameState {
            players,
            view_matrix: Default::default(),
            screen_width: 0.0,
            screen_height: 0.0,
            crosshair_entity: Some(target),
            local_view: Some(LocalView {
                eye: Vec3 { x: 0.0, y: 0.0, z: 64.0 },
                pitch: 0.0,
                yaw: 0.0,
                sensitivity: 1.0,
            }),
            smokes,
        };
        let mut visibility = Visibility::new();
        visibility.update(&state, Instant::now());
        clear_shot_at_enemy_bot(&state, &visibility)
    }

    fn aiming_at(target: u32, players: Vec<PlayerData>) -> bool {
        shot_with_smoke(target, players, vec![])
    }

    #[test]
    fn fires_at_enemy_under_the_crosshair() {
        assert!(aiming_at(200, vec![player(100, 3, false, true), player(200, 2, true, false)]));
    }

    #[test]
    fn fires_at_enemy_clients() {
        assert!(aiming_at(200, vec![player(100, 3, false, true), player(200, 2, false, false)]));
    }

    #[test]
    fn ignores_teammates() {
        assert!(!aiming_at(200, vec![player(100, 3, false, true), player(200, 3, true, false)]));
    }

    #[test]
    fn ignores_non_player_entities() {
        assert!(!aiming_at(300, vec![player(100, 3, false, true), player(200, 2, true, false)]));
    }

    #[test]
    fn needs_a_gun() {
        let mut local = player(100, 3, false, true);
        local.weapon_id = KNIFE;
        assert!(!aiming_at(200, vec![local, player(200, 2, true, false)]));
    }

    #[test]
    fn needs_a_living_local_player() {
        assert!(!aiming_at(200, vec![player(200, 2, true, false)]));
    }

    #[test]
    fn holds_fire_through_smoke() {
        let smoke = Vec3 { x: 500.0, y: 0.0, z: 32.0 };
        let players = vec![player(100, 3, false, true), player(200, 2, true, false)];
        assert!(!shot_with_smoke(200, players, vec![smoke]));
    }
}
