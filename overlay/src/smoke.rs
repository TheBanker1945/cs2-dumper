use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::game::{Offsets, ENTITY_IDENTITY_SIZE};
use crate::math::Vec3;
use crate::mem::GameProcess;

/// How far a smoke cloud reaches from its centre. CS2 smoke fills about a 144 unit
/// radius around where the grenade went off; this is a sphere, not the real volume.
const SMOKE_RADIUS: f32 = 144.0;
/// Clouds bloom up from the floor, so their centre sits above the detonation point.
const SMOKE_RISE: f32 = 32.0;

/// Networked entities (projectiles included) sit in the first chunks of the entity
/// list; client-only ones such as HUD models and lights start at index 16384.
const NETWORKED_CHUNKS: u64 = 4;
const IDENTITIES_PER_CHUNK: usize = 512;
/// Smokes don't move once they've gone off, so a few scans a second keep up.
const RESCAN_EVERY: Duration = Duration::from_millis(250);
const SMOKE_DESIGNER_NAME: &str = "smokegrenade_projectile";

/// Whether the straight line from `eye` to `target` passes through a smoke cloud.
/// Being inside a cloud blocks everything.
pub fn blocks_view(eye: Vec3, target: Vec3, clouds: &[Vec3]) -> bool {
    let line = sub(target, eye);
    let len_sq = dot(line, line);
    clouds.iter().any(|&det| {
        let centre = Vec3 { z: det.z + SMOKE_RISE, ..det };
        // Closest point on the segment to the cloud's centre.
        let t = if len_sq > 0.0 {
            (dot(sub(centre, eye), line) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let closest = Vec3 {
            x: eye.x + line.x * t,
            y: eye.y + line.y * t,
            z: eye.z + line.z * t,
        };
        let off = sub(centre, closest);
        dot(off, off) < SMOKE_RADIUS * SMOKE_RADIUS
    })
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Finds the detonation points of smoke grenades that are currently smoking.
pub struct SmokeScanner {
    /// Designer names are interned, so each distinct name pointer is only read once.
    is_smoke_name: HashMap<u64, bool>,
    clouds: Vec<Vec3>,
    scanned_at: Option<Instant>,
}

impl SmokeScanner {
    pub fn new() -> Self {
        Self {
            is_smoke_name: HashMap::new(),
            clouds: Vec::new(),
            scanned_at: None,
        }
    }

    pub fn clouds(&mut self, proc: &GameProcess, offsets: &Offsets, client_base: u64) -> Vec<Vec3> {
        if !self.scanned_at.is_some_and(|t| t.elapsed() < RESCAN_EVERY) {
            self.scanned_at = Some(Instant::now());
            self.clouds = self.scan(proc, offsets, client_base);
        }
        self.clouds.clone()
    }

    fn scan(&mut self, proc: &GameProcess, offsets: &Offsets, client_base: u64) -> Vec<Vec3> {
        let mut clouds = Vec::new();
        let Some(entity_list) = proc
            .read::<u64>(client_base + offsets.entity_list)
            .filter(|&v| v != 0)
        else {
            return clouds;
        };
        // Bounded in case names ever stop being interned.
        if self.is_smoke_name.len() > 4096 {
            self.is_smoke_name.clear();
        }

        let size = ENTITY_IDENTITY_SIZE as usize;
        let name_at = offsets.m_designer_name as usize;
        for chunk in 0..NETWORKED_CHUNKS {
            let Some(identities) = proc
                .read::<u64>(entity_list + 0x10 + 8 * chunk)
                .filter(|&v| v != 0)
                .and_then(|ptr| proc.read_bytes(ptr, IDENTITIES_PER_CHUNK * size))
            else {
                continue;
            };
            for identity in identities.chunks_exact(size) {
                let entity = read_u64(identity, 0);
                let name = read_u64(identity, name_at);
                if entity == 0 || name == 0 {
                    continue;
                }
                let is_smoke = *self
                    .is_smoke_name
                    .entry(name)
                    .or_insert_with(|| proc.read_string(name, 64) == SMOKE_DESIGNER_NAME);
                if is_smoke && proc.read::<u8>(entity + offsets.m_b_did_smoke_effect) == Some(1) {
                    if let Some([x, y, z]) =
                        proc.read::<[f32; 3]>(entity + offsets.m_v_smoke_detonation_pos)
                    {
                        clouds.push(Vec3 { x, y, z });
                    }
                }
            }
        }
        clouds
    }
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    bytes
        .get(at..at + 8)
        .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 64.0 };

    fn at(x: f32, y: f32) -> Vec3 {
        Vec3 { x, y, z: 64.0 - SMOKE_RISE }
    }

    fn target(x: f32) -> Vec3 {
        Vec3 { x, y: 0.0, z: 64.0 }
    }

    #[test]
    fn smoke_between_blocks() {
        assert!(blocks_view(EYE, target(1000.0), &[at(500.0, 0.0)]));
    }

    #[test]
    fn smoke_off_to_the_side_does_not_block() {
        assert!(!blocks_view(EYE, target(1000.0), &[at(500.0, 200.0)]));
    }

    #[test]
    fn smoke_behind_the_target_does_not_block() {
        assert!(!blocks_view(EYE, target(1000.0), &[at(1300.0, 0.0)]));
    }

    #[test]
    fn standing_in_smoke_blocks() {
        assert!(blocks_view(EYE, target(1000.0), &[at(50.0, 50.0)]));
    }

    #[test]
    fn no_smoke_no_block() {
        assert!(!blocks_view(EYE, target(1000.0), &[]));
    }
}
