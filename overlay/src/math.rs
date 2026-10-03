#[derive(Clone, Copy, Default, Debug)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub type ViewMatrix = [f32; 16];

pub fn world_to_screen(pos: Vec3, vm: &ViewMatrix, sw: f32, sh: f32) -> Option<(f32, f32)> {
    let w = vm[12] * pos.x + vm[13] * pos.y + vm[14] * pos.z + vm[15];
    if w < 0.001 {
        return None;
    }
    let x = vm[0] * pos.x + vm[1] * pos.y + vm[2] * pos.z + vm[3];
    let y = vm[4] * pos.x + vm[5] * pos.y + vm[6] * pos.z + vm[7];
    Some((
        sw / 2.0 + (x / w) * sw / 2.0,
        sh / 2.0 - (y / w) * sh / 2.0,
    ))
}
