use std::fs;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::math::{Vec3, ViewMatrix};
use crate::mem::GameProcess;

pub const MAX_BONES: usize = 28;
const BONE_ARRAY_OFFSET: u64 = 0x80;
/// Size of a CEntityIdentity entry in the entity list chunks (was 0x78 before
/// a CS2 update; with the wrong stride no players are found at all).
const ENTITY_IDENTITY_SIZE: u64 = 0x70;

pub const BONE_CONNECTIONS: &[(usize, usize)] = &[
    (6, 5),   // head -> neck
    (5, 4),   // neck -> upper spine
    (4, 2),   // upper spine -> mid spine
    (2, 0),   // mid spine -> pelvis
    (4, 8),   // upper spine -> left shoulder
    (8, 9),   // left shoulder -> left elbow
    (9, 11),  // left elbow -> left hand
    (4, 13),  // upper spine -> right shoulder
    (13, 14), // right shoulder -> right elbow
    (14, 16), // right elbow -> right hand
    (0, 22),  // pelvis -> left hip
    (22, 23), // left hip -> left knee
    (23, 24), // left knee -> left foot
    (0, 25),  // pelvis -> right hip
    (25, 26), // right hip -> right knee
    (26, 27), // right knee -> right foot
];

pub struct Offsets {
    pub entity_list: u64,
    pub local_player_controller: u64,
    pub view_matrix: u64,
    pub window_width: u64,
    pub window_height: u64,
    pub m_h_player_pawn: u64,
    pub m_i_team_num: u64,
    pub m_i_health: u64,
    pub m_life_state: u64,
    pub m_p_game_scene_node: u64,
    pub m_isz_player_name: u64,
    pub m_model_state: u64,
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
            window_width: get_u64(en, "dwWindowWidth")?,
            window_height: get_u64(en, "dwWindowHeight")?,
            m_h_player_pawn: get_field(classes, "CCSPlayerController", "m_hPlayerPawn")?,
            m_i_team_num: get_field(classes, "C_BaseEntity", "m_iTeamNum")?,
            m_i_health: get_field(classes, "C_BaseEntity", "m_iHealth")?,
            m_life_state: get_field(classes, "C_BaseEntity", "m_lifeState")?,
            m_p_game_scene_node: get_field(classes, "C_BaseEntity", "m_pGameSceneNode")?,
            m_isz_player_name: get_field(classes, "CBasePlayerController", "m_iszPlayerName")?,
            m_model_state: get_field(classes, "CSkeletonInstance", "m_modelState")?,
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
    pub index: usize,
    pub name: String,
    pub team: u8,
    pub health: i32,
    pub bones: [Vec3; MAX_BONES],
    pub is_local: bool,
}

pub struct GameState {
    pub players: Vec<PlayerData>,
    pub view_matrix: ViewMatrix,
    pub screen_width: f32,
    pub screen_height: f32,
}

pub fn read_game_state(
    proc: &GameProcess,
    offsets: &Offsets,
    client_base: u64,
    engine_base: u64,
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

    let players = read_players(proc, offsets, client_base);

    GameState {
        players,
        view_matrix,
        screen_width,
        screen_height,
    }
}

fn read_players(proc: &GameProcess, offsets: &Offsets, client_base: u64) -> Vec<PlayerData> {
    let mut players = Vec::new();

    let entity_list = match proc.read::<u64>(client_base + offsets.entity_list) {
        Some(v) if v != 0 => v,
        _ => return players,
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

        if health <= 0 || life_state != 0 {
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

        players.push(PlayerData {
            index: i as usize,
            name: if name.is_empty() {
                format!("Player {}", i)
            } else {
                name
            },
            team,
            health,
            bones,
            is_local,
        });
    }

    players
}
