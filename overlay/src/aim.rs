use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::game::{GameState, LocalView, HEAD_BONE};
use crate::math::Vec3;
use crate::trigger::enemy_player;
use crate::visibility::Visibility;
use crate::weapons::is_gun;

/// Heads within this angle of the crosshair (degrees) get picked up, so the view never
/// whips round to a target off to the side.
const ACQUIRE_FOV: f32 = 8.0;
/// A target already being tracked is kept until its head leaves this wider cone.
const KEEP_FOV: f32 = 15.0;
/// Share of the remaining angle corrected each frame. Under 1 so the crosshair glides
/// onto the head, and so a turn rate that's off (zoomed in, say) still converges.
const SMOOTHING: f32 = 0.5;
/// CS2's default `m_yaw` / `m_pitch`: degrees turned per mouse count at sensitivity 1.
const DEGREES_PER_COUNT: f32 = 0.022;
/// The crosshair counts as on the head within this distance (game units) of the
/// head bone, about the size of the head hitbox.
const HEAD_RADIUS: f32 = 4.0;
/// Longest the trigger waits for the crosshair to reach the head. A moving target can
/// keep the aim a fraction of a degree behind, and a body shot beats no shot.
const HEAD_WAIT: Duration = Duration::from_millis(150);

/// Pulls the crosshair onto the head of an enemy in clear view.
pub struct AimAssist {
    /// Pawn index of the target being tracked, and since when.
    target: Option<(u32, Instant)>,
    /// Sub-count mouse movement carried over to the next frame.
    carry: (f32, f32),
    /// The crosshair was on the tracked head, nothing was tracked, or HEAD_WAIT ran out.
    lined_up: bool,
}

impl AimAssist {
    pub fn new() -> Self {
        Self {
            target: None,
            carry: (0.0, 0.0),
            lined_up: true,
        }
    }

    /// False while still moving onto a head, so the trigger can hold fire until the
    /// shot would land there.
    pub fn lined_up(&self) -> bool {
        self.lined_up
    }

    /// Call once per frame. `active` is false unless the player is shooting (or holding
    /// the trigger key) with CS2 focused and the menu closed.
    pub fn update(&mut self, active: bool, state: &GameState, visibility: &Visibility) {
        let aim = state.local_view.filter(|_| active).and_then(|view| {
            let current = self.target.map(|(pawn, _)| pawn);
            choose_target(state, visibility, &view, current).map(|t| (view, t))
        });
        let Some((view, Target { pawn, pitch, yaw, distance })) = aim else {
            self.target = None;
            self.carry = (0.0, 0.0);
            self.lined_up = true;
            return;
        };
        let since = match self.target {
            Some((tracked, since)) if tracked == pawn => since,
            _ => Instant::now(),
        };
        self.target = Some((pawn, since));
        self.lined_up = on_head(pitch, yaw, distance) || since.elapsed() >= HEAD_WAIT;

        let (dx, dy) = mouse_counts(pitch * SMOOTHING, yaw * SMOOTHING, view.sensitivity);
        let (dx, dy) = (dx + self.carry.0, dy + self.carry.1);
        let (mx, my) = (dx.trunc(), dy.trunc());
        self.carry = (dx - mx, dy - my);
        if mx != 0.0 || my != 0.0 {
            send_move(mx as i32, my as i32);
        }
    }
}

/// A head to aim at: the (pitch, yaw) turn in degrees that puts the crosshair on it,
/// and how far away it is.
#[derive(Clone, Copy, Debug)]
struct Target {
    pawn: u32,
    pitch: f32,
    yaw: f32,
    distance: f32,
}

impl Target {
    fn off_centre(&self) -> f32 {
        self.pitch.hypot(self.yaw)
    }
}

/// The visible enemy to aim at. Sticks with `current` while it's still valid and
/// inside KEEP_FOV.
fn choose_target(
    state: &GameState,
    visibility: &Visibility,
    view: &LocalView,
    current: Option<u32>,
) -> Option<Target> {
    let local = state.local()?;
    if !is_gun(local.weapon_id) {
        return None;
    }

    let candidates: Vec<Target> = state
        .players
        .iter()
        .filter(|p| !p.is_local && enemy_player(state, local, p.pawn_index).is_some())
        .filter(|p| visibility.can_see(state, p))
        .map(|p| {
            let head = p.bones[HEAD_BONE];
            let (pitch, yaw) = turn_towards(view, head);
            let distance = distance(view.eye, head);
            Target { pawn: p.pawn_index, pitch, yaw, distance }
        })
        .collect();

    let kept = candidates
        .iter()
        .find(|t| Some(t.pawn) == current && t.off_centre() <= KEEP_FOV);
    kept.or_else(|| {
        candidates
            .iter()
            .filter(|t| t.off_centre() <= ACQUIRE_FOV)
            .min_by(|a, b| a.off_centre().total_cmp(&b.off_centre()))
    })
    .copied()
}

/// Whether a remaining turn of (pitch, yaw) degrees is within the head at `distance`.
fn on_head(pitch: f32, yaw: f32, distance: f32) -> bool {
    pitch.hypot(yaw) <= HEAD_RADIUS.atan2(distance).to_degrees()
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// The (pitch, yaw) change in degrees that points the view from the eyes at `target`.
fn turn_towards(view: &LocalView, target: Vec3) -> (f32, f32) {
    let (dx, dy, dz) = (
        target.x - view.eye.x,
        target.y - view.eye.y,
        target.z - view.eye.z,
    );
    let yaw = dy.atan2(dx).to_degrees();
    let pitch = -dz.atan2(dx.hypot(dy)).to_degrees();
    (pitch - view.pitch, wrap_degrees(yaw - view.yaw))
}

/// Into -180..180, so the turn goes the short way round.
fn wrap_degrees(deg: f32) -> f32 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}

/// Mouse counts for a view turn: moving right turns right (yaw goes down) and moving
/// down looks down (pitch goes up).
fn mouse_counts(pitch: f32, yaw: f32, sensitivity: f32) -> (f32, f32) {
    let per_degree = 1.0 / (sensitivity * DEGREES_PER_COUNT);
    (-yaw * per_degree, pitch * per_degree)
}

fn send_move(dx: i32, dy: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                dwFlags: MOUSEEVENTF_MOVE,
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
    use crate::game::{PlayerData, MAX_BONES};

    const AK47: u16 = 7;
    const KNIFE: u16 = 42;
    const EYE: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 64.0 };

    fn view(pitch: f32, yaw: f32) -> LocalView {
        LocalView {
            eye: EYE,
            pitch,
            yaw,
            sensitivity: 1.0,
        }
    }

    fn local() -> PlayerData {
        player(100, 3, false, Vec3::default())
    }

    /// An enemy pawn spotted by the local player, controller index `pawn_index / 100`,
    /// with its head at `head`. `is_bot` is set verbatim on the record.
    fn player(pawn_index: u32, team: u8, is_bot: bool, head: Vec3) -> PlayerData {
        let mut bones = [Vec3::default(); MAX_BONES];
        bones[HEAD_BONE] = head;
        PlayerData {
            name: String::new(),
            team,
            health: 100,
            bones,
            is_local: pawn_index == 100,
            weapon_id: AK47,
            ammo: 30,
            index: pawn_index / 100,
            pawn_index,
            is_bot,
            spotted_by: 1,
        }
    }

    /// A head 1000 units away, `pitch`/`yaw` degrees off straight ahead along +x.
    fn head_at(pitch: f32, yaw: f32) -> Vec3 {
        let (p, y) = (pitch.to_radians(), yaw.to_radians());
        Vec3 {
            x: EYE.x + 1000.0 * p.cos() * y.cos(),
            y: EYE.y + 1000.0 * p.cos() * y.sin(),
            z: EYE.z - 1000.0 * p.sin(),
        }
    }

    fn state(players: Vec<PlayerData>) -> GameState {
        GameState {
            players,
            view_matrix: Default::default(),
            screen_width: 0.0,
            screen_height: 0.0,
            crosshair_entity: None,
            local_view: Some(view(0.0, 0.0)),
            smokes: Vec::new(),
        }
    }

    fn choose(state: &GameState, current: Option<u32>) -> Option<Target> {
        let mut visibility = Visibility::new();
        visibility.update(state, std::time::Instant::now());
        choose_target(state, &visibility, &view(0.0, 0.0), current)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn turn_towards_matches_cs2_angle_conventions() {
        let (pitch, yaw) = turn_towards(&view(0.0, 0.0), head_at(-10.0, 30.0));
        assert!(close(pitch, -10.0) && close(yaw, 30.0), "{pitch} {yaw}");
    }

    #[test]
    fn turn_takes_the_short_way_round() {
        let (_, yaw) = turn_towards(&view(0.0, 170.0), head_at(0.0, -170.0));
        assert!(close(yaw, 20.0), "{yaw}");
    }

    #[test]
    fn mouse_moves_towards_the_target() {
        // Target to the right (yaw down) and below (pitch up): move right and down.
        let (dx, dy) = mouse_counts(2.0, -3.0, 2.0);
        assert!(close(dx, 3.0 / 0.044) && close(dy, 2.0 / 0.044), "{dx} {dy}");
    }

    #[test]
    fn picks_the_head_nearest_the_crosshair() {
        let s = state(vec![
            local(),
            player(200, 2, true, head_at(0.0, 5.0)),
            player(300, 2, true, head_at(1.0, -2.0)),
        ]);
        let t = choose(&s, None).unwrap();
        assert_eq!(t.pawn, 300);
        assert!(close(t.pitch, 1.0) && close(t.yaw, -2.0) && close(t.distance, 1000.0));
    }

    #[test]
    fn ignores_heads_outside_the_fov() {
        let s = state(vec![local(), player(200, 2, true, head_at(0.0, 20.0))]);
        assert!(choose(&s, None).is_none());
    }

    #[test]
    fn never_aims_at_teammates_or_hidden_enemies() {
        let mut hidden = player(400, 2, true, head_at(0.0, 1.0));
        hidden.spotted_by = 0;
        let s = state(vec![
            local(),
            player(300, 3, true, head_at(0.0, 1.0)),
            hidden,
        ]);
        assert!(choose(&s, None).is_none());
    }

    #[test]
    fn aims_at_non_bot_enemies() {
        let s = state(vec![
            local(),
            player(200, 2, false, head_at(0.0, 1.0)),
        ]);
        assert!(choose(&s, None).is_some());
    }

    #[test]
    fn needs_a_gun() {
        let mut me = local();
        me.weapon_id = KNIFE;
        let s = state(vec![me, player(200, 2, true, head_at(0.0, 1.0))]);
        assert!(choose(&s, None).is_none());
    }

    #[test]
    fn aims_through_clear_air_only() {
        let mut s = state(vec![local(), player(200, 2, true, head_at(0.0, 1.0))]);
        s.smokes.push(Vec3 { x: 500.0, y: 0.0, z: 32.0 });
        assert!(choose(&s, None).is_none());
    }

    #[test]
    fn on_head_scales_with_distance() {
        // A 4 unit head covers about 0.23 degrees at 1000 units and 2.3 at 100.
        assert!(on_head(0.0, 0.2, 1000.0));
        assert!(!on_head(0.0, 0.5, 1000.0));
        assert!(on_head(1.0, 1.5, 100.0));
    }

    #[test]
    fn keeps_tracking_its_target_inside_the_wider_cone() {
        let s = state(vec![
            local(),
            player(200, 2, true, head_at(0.0, 12.0)),
            player(300, 2, true, head_at(0.0, 1.0)),
        ]);
        assert_eq!(choose(&s, Some(200)).unwrap().pawn, 200);
    }

    #[test]
    fn lets_go_once_the_target_leaves_the_wider_cone() {
        let s = state(vec![
            local(),
            player(200, 2, true, head_at(0.0, 16.0)),
            player(300, 2, true, head_at(0.0, 1.0)),
        ]);
        assert_eq!(choose(&s, Some(200)).unwrap().pawn, 300);
    }
}
