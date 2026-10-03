use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::*;

use crate::game::{PlayerData, BONE_CONNECTIONS, MAX_BONES};
use crate::math::{world_to_screen, ViewMatrix};

const COLORKEY: COLORREF = COLORREF(0x00FF00FF);

const CT_COLOR: COLORREF = COLORREF(0x00FFB478);
const T_COLOR: COLORREF = COLORREF(0x0050C8FF);
const CT_COLOR_DIM: COLORREF = COLORREF(0x00CC8050);
const T_COLOR_DIM: COLORREF = COLORREF(0x003090CC);

const MENU_BG: COLORREF = COLORREF(0x00281E1E);
const MENU_HEADER_BG: COLORREF = COLORREF(0x00322323);
const MENU_ACCENT: COLORREF = COLORREF(0x00FF7864);
const MENU_TEXT: COLORREF = COLORREF(0x00E6E6E6);
const MENU_SUBTEXT: COLORREF = COLORREF(0x00A09696);
const TOGGLE_ON: COLORREF = COLORREF(0x0078C850);
const TOGGLE_OFF: COLORREF = COLORREF(0x005A5050);
const TOGGLE_KNOB: COLORREF = COLORREF(0x00F0F0F0);

const MENU_X: i32 = 40;
const MENU_Y: i32 = 40;
const MENU_W: i32 = 320;
const ROW_H: i32 = 34;
const HEADER_H: i32 = 52;

pub struct ToggleArea {
    pub player_index: usize,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub struct Renderer {
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    font: HFONT,
    font_title: HFONT,
    font_small: HFONT,
    pub width: i32,
    pub height: i32,
    pub toggle_areas: Vec<ToggleArea>,
    pub toggle_all_rect: (i32, i32, i32, i32),
}

impl Renderer {
    pub fn new(width: i32, height: i32) -> Self {
        unsafe {
            let screen_dc = GetDC(HWND::default());
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap);
            ReleaseDC(HWND::default(), screen_dc);

            let font = create_font(-16, false);
            let font_title = create_font(-20, true);
            let font_small = create_font(-13, false);

            Self {
                mem_dc,
                bitmap,
                old_bitmap,
                font,
                font_title,
                font_small,
                width,
                height,
                toggle_areas: Vec::new(),
                toggle_all_rect: (0, 0, 0, 0),
            }
        }
    }

    pub fn begin_frame(&self) {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            let brush = CreateSolidBrush(COLORKEY);
            FillRect(self.mem_dc, &rect, brush);
            let _ = DeleteObject(brush);
        }
    }

    pub fn end_frame(&self, hwnd: HWND) {
        unsafe {
            let hdc = GetDC(hwnd);
            let _ = BitBlt(hdc, 0, 0, self.width, self.height, self.mem_dc, 0, 0, SRCCOPY);
            ReleaseDC(hwnd, hdc);
        }
    }

    pub fn draw_skeleton(&self, player: &PlayerData, vm: &ViewMatrix, sw: f32, sh: f32) {
        let (color, shadow_color) = if player.team == 3 {
            (CT_COLOR, CT_COLOR_DIM)
        } else {
            (T_COLOR, T_COLOR_DIM)
        };

        let dc = self.mem_dc;

        let screen_bones: Vec<Option<(f32, f32)>> = (0..MAX_BONES)
            .map(|i| world_to_screen(player.bones[i], vm, sw, sh))
            .collect();

        unsafe {
            let shadow_pen = CreatePen(PS_SOLID, 4, shadow_color);
            let old = SelectObject(dc, shadow_pen);
            draw_bone_lines(dc, &screen_bones);
            SelectObject(dc, old);
            let _ = DeleteObject(shadow_pen);

            let pen = CreatePen(PS_SOLID, 2, color);
            let old = SelectObject(dc, pen);
            draw_bone_lines(dc, &screen_bones);
            SelectObject(dc, old);
            let _ = DeleteObject(pen);

            if let Some((hx, hy)) = screen_bones[6] {
                let r = 6;
                let outline_pen = CreatePen(PS_SOLID, 2, color);
                let null_brush = GetStockObject(NULL_BRUSH);
                let old_pen = SelectObject(dc, outline_pen);
                let old_brush = SelectObject(dc, null_brush);
                let _ = Ellipse(dc, hx as i32 - r, hy as i32 - r, hx as i32 + r, hy as i32 + r);
                SelectObject(dc, old_pen);
                SelectObject(dc, old_brush);
                let _ = DeleteObject(outline_pen);
            }

            if let (Some((_, head_y)), Some((_, pelvis_y))) = (screen_bones[6], screen_bones[0]) {
                let top_y = head_y.min(pelvis_y) as i32 - 8;
                let bot_y = head_y.max(pelvis_y) as i32 + 8;
                let bar_h = bot_y - top_y;
                if bar_h > 5 {
                    let bar_x = screen_bones
                        .iter()
                        .filter_map(|b| b.map(|(x, _)| x as i32))
                        .min()
                        .unwrap_or(0)
                        - 10;

                    let bg_brush = CreateSolidBrush(COLORREF(0x00333333));
                    let bg_rect = RECT { left: bar_x - 4, top: top_y, right: bar_x, bottom: bot_y };
                    FillRect(dc, &bg_rect, bg_brush);
                    let _ = DeleteObject(bg_brush);

                    let hp = (player.health as f32 / 100.0).clamp(0.0, 1.0);
                    let r = ((1.0 - hp) * 255.0) as u32;
                    let g = (hp * 255.0) as u32;
                    let fill_color = COLORREF(r | (g << 8));
                    let fill_h = (bar_h as f32 * hp) as i32;
                    let fill_brush = CreateSolidBrush(fill_color);
                    let fill_rect = RECT { left: bar_x - 4, top: bot_y - fill_h, right: bar_x, bottom: bot_y };
                    FillRect(dc, &fill_rect, fill_brush);
                    let _ = DeleteObject(fill_brush);
                }
            }

            if let Some((hx, hy)) = screen_bones[6] {
                let old_font = SelectObject(dc, self.font_small);
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, color);
                let label = format!("{} [{}]", player.name, player.health);
                let mut wide: Vec<u16> = label.encode_utf16().collect();
                let mut rc = RECT {
                    left: hx as i32 - 100,
                    top: hy as i32 - 24,
                    right: hx as i32 + 100,
                    bottom: hy as i32 - 8,
                };
                DrawTextW(dc, &mut wide, &mut rc, DT_CENTER | DT_NOCLIP);
                SelectObject(dc, old_font);
            }
        }
    }

    pub fn draw_menu(&mut self, players: &[PlayerData], visible: &[bool; 65]) {
        self.toggle_areas.clear();
        let dc = self.mem_dc;

        let player_count = players.len() as i32;
        let footer_h = 40;
        let toggle_all_h = ROW_H;
        let menu_h = HEADER_H + 2 + toggle_all_h + player_count * ROW_H + footer_h;

        unsafe {
            let bg = CreateSolidBrush(MENU_BG);
            let rect = RECT { left: MENU_X, top: MENU_Y, right: MENU_X + MENU_W, bottom: MENU_Y + menu_h };
            FillRect(dc, &rect, bg);
            let _ = DeleteObject(bg);

            let hdr = CreateSolidBrush(MENU_HEADER_BG);
            let hdr_rect = RECT { left: MENU_X, top: MENU_Y, right: MENU_X + MENU_W, bottom: MENU_Y + HEADER_H };
            FillRect(dc, &hdr_rect, hdr);
            let _ = DeleteObject(hdr);

            let accent = CreateSolidBrush(MENU_ACCENT);
            let acc_rect = RECT { left: MENU_X, top: MENU_Y + HEADER_H, right: MENU_X + MENU_W, bottom: MENU_Y + HEADER_H + 2 };
            FillRect(dc, &acc_rect, accent);
            let _ = DeleteObject(accent);

            SetBkMode(dc, TRANSPARENT);

            let old_font = SelectObject(dc, self.font_title);
            SetTextColor(dc, MENU_TEXT);
            let mut title: Vec<u16> = "CS2 SKELETON ESP".encode_utf16().collect();
            let mut title_rc = RECT { left: MENU_X + 16, top: MENU_Y + 8, right: MENU_X + MENU_W - 16, bottom: MENU_Y + 30 };
            DrawTextW(dc, &mut title, &mut title_rc, DT_LEFT | DT_NOCLIP);

            SelectObject(dc, self.font_small);
            SetTextColor(dc, MENU_SUBTEXT);
            let mut sub: Vec<u16> = "Educational Purpose Only".encode_utf16().collect();
            let mut sub_rc = RECT { left: MENU_X + 16, top: MENU_Y + 32, right: MENU_X + MENU_W - 16, bottom: MENU_Y + HEADER_H };
            DrawTextW(dc, &mut sub, &mut sub_rc, DT_LEFT | DT_NOCLIP);

            let ta_y = MENU_Y + HEADER_H + 2;
            SelectObject(dc, self.font);
            SetTextColor(dc, MENU_TEXT);
            let mut ta_text: Vec<u16> = "Toggle All".encode_utf16().collect();
            let mut ta_rc = RECT { left: MENU_X + 16, top: ta_y + 6, right: MENU_X + 200, bottom: ta_y + ROW_H };
            DrawTextW(dc, &mut ta_text, &mut ta_rc, DT_LEFT | DT_NOCLIP);

            let any_on = players.iter().any(|p| visible[p.index]);
            let ta_toggle_x = MENU_X + MENU_W - 66;
            draw_toggle(dc, ta_toggle_x, ta_y + 7, any_on);
            self.toggle_all_rect = (MENU_X, ta_y, MENU_X + MENU_W, ta_y + toggle_all_h);

            let rows_start = ta_y + toggle_all_h;
            for (row, player) in players.iter().enumerate() {
                let y = rows_start + row as i32 * ROW_H;

                let sep = CreateSolidBrush(COLORREF(0x00382828));
                let sep_rect = RECT { left: MENU_X + 12, top: y, right: MENU_X + MENU_W - 12, bottom: y + 1 };
                FillRect(dc, &sep_rect, sep);
                let _ = DeleteObject(sep);

                let dot_color = if player.team == 3 { CT_COLOR } else { T_COLOR };
                let dot_brush = CreateSolidBrush(dot_color);
                let null_pen = GetStockObject(NULL_PEN);
                let old_pen = SelectObject(dc, null_pen);
                let old_br = SelectObject(dc, dot_brush);
                let _ = Ellipse(dc, MENU_X + 16, y + 11, MENU_X + 24, y + 19);
                SelectObject(dc, old_pen);
                SelectObject(dc, old_br);
                let _ = DeleteObject(dot_brush);

                SelectObject(dc, self.font);
                SetTextColor(dc, MENU_TEXT);
                let mut name_str: Vec<u16> = player.name.chars().take(16).collect::<String>().encode_utf16().collect();
                let mut name_rc = RECT { left: MENU_X + 30, top: y + 6, right: MENU_X + 180, bottom: y + ROW_H };
                DrawTextW(dc, &mut name_str, &mut name_rc, DT_LEFT | DT_NOCLIP);

                SetTextColor(dc, MENU_SUBTEXT);
                let mut hp_str: Vec<u16> = format!("{}hp", player.health).encode_utf16().collect();
                let mut hp_rc = RECT { left: MENU_X + 185, top: y + 6, right: MENU_X + 230, bottom: y + ROW_H };
                DrawTextW(dc, &mut hp_str, &mut hp_rc, DT_LEFT | DT_NOCLIP);

                let is_on = visible[player.index];
                let tx = MENU_X + MENU_W - 66;
                let ty = y + 7;
                draw_toggle(dc, tx, ty, is_on);

                self.toggle_areas.push(ToggleArea {
                    player_index: player.index,
                    left: MENU_X,
                    top: y,
                    right: MENU_X + MENU_W,
                    bottom: y + ROW_H,
                });
            }

            let fy = rows_start + player_count * ROW_H + 8;
            SelectObject(dc, self.font_small);
            SetTextColor(dc, MENU_SUBTEXT);
            let mut hint: Vec<u16> = "INSERT: Toggle Menu  |  END: Exit".encode_utf16().collect();
            let mut hint_rc = RECT { left: MENU_X + 16, top: fy, right: MENU_X + MENU_W - 16, bottom: fy + 20 };
            DrawTextW(dc, &mut hint, &mut hint_rc, DT_CENTER | DT_NOCLIP);

            SelectObject(dc, old_font);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem_dc, self.old_bitmap);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.mem_dc);
            let _ = DeleteObject(self.font);
            let _ = DeleteObject(self.font_title);
            let _ = DeleteObject(self.font_small);
        }
    }
}

unsafe fn draw_bone_lines(dc: HDC, screen_bones: &[Option<(f32, f32)>]) {
    for &(from, to) in BONE_CONNECTIONS {
        if from >= screen_bones.len() || to >= screen_bones.len() {
            continue;
        }
        if let (Some((fx, fy)), Some((tx, ty))) = (screen_bones[from], screen_bones[to]) {
            let _ = MoveToEx(dc, fx as i32, fy as i32, None);
            let _ = LineTo(dc, tx as i32, ty as i32);
        }
    }
}

unsafe fn draw_toggle(dc: HDC, x: i32, y: i32, on: bool) {
    let w = 46;
    let h = 20;

    let bg_color = if on { TOGGLE_ON } else { TOGGLE_OFF };
    let bg_brush = CreateSolidBrush(bg_color);
    let null_pen = GetStockObject(NULL_PEN);
    let old_pen = SelectObject(dc, null_pen);
    let old_brush = SelectObject(dc, bg_brush);
    let _ = RoundRect(dc, x, y, x + w, y + h, h, h);
    SelectObject(dc, old_brush);
    let _ = DeleteObject(bg_brush);

    let knob_brush = CreateSolidBrush(TOGGLE_KNOB);
    SelectObject(dc, knob_brush);
    let kx = if on { x + w - h + 2 } else { x + 2 };
    let _ = Ellipse(dc, kx, y + 2, kx + h - 4, y + h - 2);
    SelectObject(dc, old_pen);
    SelectObject(dc, knob_brush);
    let _ = DeleteObject(knob_brush);
}

unsafe fn create_font(height: i32, bold: bool) -> HFONT {
    let weight = if bold { 700 } else { 400 };
    CreateFontW(
        height, 0, 0, 0,
        weight, 0, 0, 0,
        1, 0, 0, 5, 0,
        w!("Segoe UI"),
    )
}
