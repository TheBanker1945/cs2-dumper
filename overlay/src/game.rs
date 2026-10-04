use std::fs;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::math::{Vec3, ViewMatrix};
use crate::mem::GameProcess;
use crate::smoke::SmokeScanner;

/// Bones read per player: everything the skeleton uses (0..=22). Higher indices
/// are helpers, e.g. bone 27 is a look target ~1000 units in front of the eyes.
pub const MAX_BONES: usize = 23;
const BONE_ARRAY_OFFSET: u64 = 0x80;
/// Size of a CEntityIdentity entry in the entity list chunks (was 0x78 before
/// a CS2 update; with the wrong stride no players are found at all).
pub const ENTITY_IDENTITY_SIZE: u64 = 0x70;
/// Controller `m_fFlags` bit set on bots.
const FL_FAKECLIENT: u32 = 1 << 8;

/// Model root, on the ground between the feet.
pub const ROOT_BONE: usize = 0;
pub const HEAD_BONE: usize = 7;

// Player model bone layout, checked against live bone positions after a CS2
// update reshuffled it (bone 0 became the root, the legs moved to 17..=22).
pub const BONE_CONNECTIONS: &[(usize, usize)] = &[
    (7, 6),   // head -> neck
    (6, 5),   // neck -> upper chest
    (5, 4),   // upper chest -> chest
    (4, 3),   // chest -> lower spine
    (3, 2),   // lower spine -> spine base
    (2, 1),   // spine base -> pelvis
    (5, 8),   // upper chest -> left clavicle
    (8, 9),   // left clavicle -> left shoulder
    (9, 10),  // left shoulder -> left elbow
    (10, 11), // left elbow -> left hand
    (5, 12),  // upper chest -> right clavicle
    (12, 13), // right clavicle -> right shoulder
    (13, 14), // right shoulder -> right elbow
    (14, 15), // right elbow -> right hand
    (1, 17),  // pelvis -> left hip
    (17, 18), // left hip -> left knee
    (18, 19), // left knee -> left foot
    (1, 20),  // pelvis -> right hip
    (20, 21), // right hip -> right knee
    (21, 22), // right knee -> right foot
];

pub struct Offsets {
    pub entity_list: u64,
    pub local_player_controller: u64,
    pub view_matrix: u64,
    pub view_angles: u64,
    /// Pointer to the object holding the `sensitivity` convar, and the value's offset in it.
    pub sensitivity: u64,
    pub sensitivity_value: u64,
    pub window_width: u64,
    pub window_height: u64,
    pub m_h_player_pawn: u64,
    pub m_i_team_num: u64,
    pub m_i_health: u64,
    pub m_life_state: u64,
    pub m_p_game_scene_node: u64,
    pub m_v_old_origin: u64,
    pub m_vec_view_offset: u64,
    pub m_isz_player_name: u64,
    pub m_model_state: u64,
    pub m_p_weapon_services: u64,
    pub m_h_active_weapon: u64,
    /// C_EconEntity::m_AttributeManager + m_Item + m_iItemDefinitionIndex.
    pub item_definition_index: u64,
    pub m_i_clip1: u64,
    pub m_h_controller: u64,
    pub m_i_id_ent_index: u64,
    pub m_steam_id: u64,
    pub m_f_flags: u64,
    /// CEntityIdentity::m_designerName, e.g. "smokegrenade_projectile".
    pub m_designer_name: u64,
    pub m_b_did_smoke_effect: u64,
    pub m_v_smoke_detonation_pos: u64,
    /// C_CSPlayerPawn::m_entitySpottedState + m_bSpottedByMask.
    pub spotted_by_mask: u64,
}

impl Offsets {
    pub fn load(output_dir: &str) -> Result<Self> {
        let offsets: Value = serde_json::from_str(
            &fs::read_to_string(format!("{}/offsets.json", output_dir))
                .context("Run the dumper first to generate output/offsets.json")?,
        )?;
        let client: Value = serde_json::from_str(
            &fs::read_to_string(format!("{}/client_dll.json", output_dir))
                .context("Run the dumper first to generate output/client_dll.json")?,
        )?;

        let cl = &offsets["client.dll"];
        let en = &offsets["engine2.dll"];
        let classes = &client["client.dll"]["classes"];

        Ok(Self {
            entity_list: get_u64(cl, "dwEntityList")?,
            local_player_controller: get_u64(cl, "dwLocalPlayerController")?,
            view_matrix: get_u64(cl, "dwViewMatrix")?,
            view_angles: get_u64(cl, "dwViewAngles")?,
            sensitivity: get_u64(cl, "dwSensitivity")?,
            sensitivity_value: get_u64(cl, "dwSensitivity_sensitivity")?,
            window_width: get_u64(en, "dwWindowWidth")?,
            window_height: get_u64(en, "dwWindowHeight")?,
            m_h_player_pawn: get_field(classes, "CCSPlayerController", "m_hPlayerPawn")?,
            m_i_team_num: get_field(classes, "C_BaseEntity", "m_iTeamNum")?,
            m_i_health: get_field(classes, "C_BaseEntity", "m_iHealth")?,
            m_life_state: get_field(classes, "C_BaseEntity", "m_lifeState")?,
            m_p_game_scene_node: get_field(classes, "C_BaseEntity", "m_pGameSceneNode")?,
            m_v_old_origin: get_field(classes, "C_BasePlayerPawn", "m_vOldOrigin")?,
            m_vec_view_offset: get_field(classes, "C_BaseModelEntity", "m_vecViewOffset")?,
            m_isz_player_name: get_field(classes, "CBasePlayerController", "m_iszPlayerName")?,
            m_model_state: get_field(classes, "CSkeletonInstance", "m_modelState")?,
            m_p_weapon_services: get_field(classes, "C_BasePlayerPawn", "m_pWeaponServices")?,
            m_h_active_weapon: get_field(classes, "CPlayer_WeaponServices", "m_hActiveWeapon")?,
            item_definition_index: get_field(classes, "C_EconEntity", "m_AttributeManager")?
                + get_field(classes, "C_AttributeContainer", "m_Item")?
                + get_field(classes, "C_EconItemView", "m_iItemDefinitionIndex")?,
            m_i_clip1: get_field(classes, "C_BasePlayerWeapon", "m_iClip1")?,
            m_h_controller: get_field(classes, "C_BasePlayerPawn", "m_hController")?,
            m_i_id_ent_index: get_field(classes, "C_CSPlayerPawn", "m_iIDEntIndex")?,
            m_steam_id: get_field(classes, "CBasePlayerController", "m_steamID")?,
            m_f_flags: get_field(classes, "C_BaseEntity", "m_fFlags")?,
            m_designer_name: get_field(classes, "CEntityIdentity", "m_designerName")?,
            m_b_did_smoke_effect: get_field(classes, "C_SmokeGrenadeProjectile", "m_bDidSmokeEffect")?,
            m_v_smoke_detonation_pos: get_field(
                classes,
                "C_SmokeGrenadeProjectile",
                "m_vSmokeDetonationPos",
            )?,
            spotted_by_mask: get_field(classes, "C_CSPlayerPawn", "m_entitySpottedState")?
                + get_field(classes, "EntitySpottedState_t", "m_bSpottedByMask")?,
        })
    }
}

fn get_u64(obj: &Value, key: &str) -> Result<u64> {
    obj[key].as_u64().context(format!("missing offset: {}", key))
}

fn get_field(classes: &Value, class: &str, field: &str) -> Result<u64> {
    classes[class]["fields"][field]
        .as_u64()
        .context(format!("missing field: {}.{}", class, field))
}

#[derive(Clone)]
pub struct PlayerData {
    pub name: String,
    pub team: u8,
    pub health: i32,
    pub bones: [Vec3; MAX_BONES],
    pub is_local: bool,
    /// Item definition index of the held weapon (0 if unknown) and its magazine ammo.
    pub weapon_id: u16,
    pub ammo: i32,
    /// Entity index of the player's controller.
    pub index: u32,
    /// Entity index of the player's pawn.
    pub pawn_index: u32,
    /// A bot still driving its own pawn (not taken over by a client).
    pub is_bot: bool,
    /// Players the server sees as having line of sight to this one, as bit `index - 1`
    /// per spotter. Only enemies ever spot.
    pub spotted_by: u64,
}

pub struct GameState {
    pub players: Vec<PlayerData>,
    pub view_matrix: ViewMatrix,
    pub screen_width: f32,
    pub screen_height: f32,
    /// Entity index under the local player's crosshair, if any.
    pub crosshair_entity: Option<u32>,
    /// None while the local player isn't alive in a match.
    pub local_view: Option<LocalView>,
    /// Detonation points of smoke grenades that are smoking right now.
    pub smokes: Vec<Vec3>,
}

impl GameState {
    /// The local player, listed only while alive.
    pub fn local(&self) -> Option<&PlayerData> {
        self.players.iter().find(|p| p.is_local)
    }
}

#[derive(Clone, Copy)]
pub struct LocalView {
    pub eye: Vec3,
    /// View angles in degrees: pitch is positive looking down, yaw positive turning left.
    pub pitch: f32,
    pub yaw: f32,
    /// The `sensitivity` convar, which scales how far a mouse count turns the view.
    pub sensitivity: f32,
}

pub fn read_game_state(
    proc: &GameProcess,
    offsets: &Offsets,
    client_base: u64,
    engine_base: u64,
    smoke_scanner: &mut SmokeScanner,
) -> GameState {
    let view_matrix = proc
        .read::<ViewMatrix>(client_base + offsets.view_matrix)
        .unwrap_or_default();
    let screen_width = proc
        .read::<u32>(engine_base + offsets.window_width)
        .unwrap_or(1920) as f32;
    let screen_height = proc
        .read::<u32>(engine_base + offsets.window_height)
        .unwrap_or(1080) as f32;

    let (players, local_pawn) = read_players(proc, offsets, client_base);

    // Set by the game's own trace from the eyes, which walls stop: -1 when nothing
    // (or only the world) is under the crosshair.
    let crosshair_entity = local_pawn
        .and_then(|pawn| proc.read::<i32>(pawn + offsets.m_i_id_ent_index))
        .filter(|&id| id > 0)
        .map(|id| id as u32);
    let local_view = local_pawn.and_then(|pawn| read_local_view(proc, offsets, client_base, pawn));

    GameState {
        players,
        view_matrix,
        screen_width,
        screen_height,
        crosshair_entity,
        local_view,
        smokes: smoke_scanner.clouds(proc, offsets, client_base),
    }
}

/// Alive players on T/CT, and the local player's pawn if they're one of them.
fn read_players(
    proc: &GameProcess,
    offsets: &Offsets,
    client_base: u64,
) -> (Vec<PlayerData>, Option<u64>) {
    let mut players = Vec::new();
    let mut local_pawn = None;

    let entity_list = match proc.read::<u64>(client_base + offsets.entity_list) {
        Some(v) if v != 0 => v,
        _ => return (players, local_pawn),
    };

    let local_controller = proc
        .read::<u64>(client_base + offsets.local_player_controller)
        .unwrap_or(0);

    for i in 1u64..=64 {
        let list_entry =
            match proc.read::<u64>(entity_list + 0x8 * ((i & 0x7FFF) >> 9) + 0x10) {
                Some(v) if v != 0 => v,
                _ => continue,
            };

        let controller = match proc.read::<u64>(list_entry + ENTITY_IDENTITY_SIZE * (i & 0x1FF)) {
            Some(v) if v != 0 => v,
            _ => continue,
        };

        let is_local = controller == local_controller;

        let name = proc.read_string(controller + offsets.m_isz_player_name, 128);

        let pawn_handle = match proc.read::<u32>(controller + offsets.m_h_player_pawn) {
            Some(v) if v != 0xFFFFFFFF => v,
            _ => continue,
        };

        let pawn_entry = match proc
            .read::<u64>(entity_list + 0x8 * ((pawn_handle as u64 & 0x7FFF) >> 9) + 0x10)
        {
            Some(v) if v != 0 => v,
            _ => continue,
        };

        let pawn =
            match proc.read::<u64>(pawn_entry + ENTITY_IDENTITY_SIZE * (pawn_handle as u64 & 0x1FF)) {
                Some(v) if v != 0 => v,
                _ => continue,
            };

        let team = proc.read::<u8>(pawn + offsets.m_i_team_num).unwrap_or(0);
        let health = proc.read::<i32>(pawn + offsets.m_i_health).unwrap_or(0);
        let life_state = proc.read::<u8>(pawn + offsets.m_life_state).unwrap_or(1);

        // Teams 0/1 are unassigned and spectators.
        if health <= 0 || life_state != 0 || !(team == 2 || team == 3) {
            continue;
        }

        let game_scene_node = match proc.read::<u64>(pawn + offsets.m_p_game_scene_node) {
            Some(v) if v != 0 => v,
            _ => continue,
        };

        let bone_array_ptr = match proc
            .read::<u64>(game_scene_node + offsets.m_model_state as u64 + BONE_ARRAY_OFFSET)
        {
            Some(v) if v != 0 => v,
            _ => continue,
        };

        let mut bones = [Vec3::default(); MAX_BONES];
        for idx in 0..MAX_BONES {
            if let Some(pos) = proc.read::<[f32; 3]>(bone_array_ptr + (idx as u64) * 32) {
                bones[idx] = Vec3 {
                    x: pos[0],
                    y: pos[1],
                    z: pos[2],
                };
            }
        }

        let (weapon_id, ammo) =
            read_active_weapon(proc, offsets, entity_list, pawn).unwrap_or((0, -1));

        if is_local {
            local_pawn = Some(pawn);
        }

        // Bots have no Steam ID and carry FL_FAKECLIENT. A client taking over a bot
        // becomes its pawn's controller, so the pawn must also be driven by this controller.
        // Unreadable values count as a client, so a failed read can never mark a bot.
        let fake_client = proc.read::<u64>(controller + offsets.m_steam_id) == Some(0)
            && proc
                .read::<u32>(controller + offsets.m_f_flags)
                .is_some_and(|flags| flags & FL_FAKECLIENT != 0);
        let driver = proc.read::<u32>(pawn + offsets.m_h_controller).unwrap_or(0) as u64 & 0x7FFF;
        let is_bot = fake_client && driver == i;

        // Unreadable counts as not spotted, so a failed read can never clear a shot.
        let spotted_by = proc.read::<u64>(pawn + offsets.spotted_by_mask).unwrap_or(0);

        players.push(PlayerData {
            name: if name.is_empty() {
                format!("Player {}", i)
            } else {
                name
            },
            team,
            health,
            bones,
            is_local,
            weapon_id,
            ammo,
            index: i as u32,
            pawn_index: pawn_handle & 0x7FFF,
            is_bot,
            spotted_by,
        });
    }

    (players, local_pawn)
}

/// Where the local player's eyes are and where they're looking.
fn read_local_view(
    proc: &GameProcess,
    offsets: &Offsets,
    client_base: u64,
    pawn: u64,
) -> Option<LocalView> {
    let [x, y, z] = proc.read::<[f32; 3]>(pawn + offsets.m_v_old_origin)?;
    let [ox, oy, oz] = proc.read::<[f32; 3]>(pawn + offsets.m_vec_view_offset)?;
    let [pitch, yaw, _roll] = proc.read::<[f32; 3]>(client_base + offsets.view_angles)?;
    let sensitivity = proc
        .read::<u64>(client_base + offsets.sensitivity)
        .filter(|&p| p != 0)
        .and_then(|p| proc.read::<f32>(p + offsets.sensitivity_value))
        .filter(|&s| s > 0.0)?;
    Some(LocalView {
        eye: Vec3 { x: x + ox, y: y + oy, z: z + oz },
        pitch,
        yaw,
        sensitivity,
    })
}

/// Item definition index and magazine ammo of the weapon the pawn is holding.
fn read_active_weapon(
    proc: &GameProcess,
    offsets: &Offsets,
    entity_list: u64,
    pawn: u64,
) -> Option<(u16, i32)> {
    let services = proc.read::<u64>(pawn + offsets.m_p_weapon_services).filter(|&v| v != 0)?;
    let handle = proc
        .read::<u32>(services + offsets.m_h_active_weapon)
        .filter(|&h| h != 0xFFFFFFFF)? as u64;
    let entry = proc
        .read::<u64>(entity_list + 0x8 * ((handle & 0x7FFF) >> 9) + 0x10)
        .filter(|&v| v != 0)?;
    let weapon = proc
        .read::<u64>(entry + ENTITY_IDENTITY_SIZE * (handle & 0x1FF))
        .filter(|&v| v != 0)?;
    let id = proc.read::<u16>(weapon + offsets.item_definition_index)?;
    let ammo = proc.read::<i32>(weapon + offsets.m_i_clip1).unwrap_or(-1);
    Some((id, ammo))
}
