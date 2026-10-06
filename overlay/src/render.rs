use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::*;

use crate::game::{PlayerData, BONE_CONNECTIONS, HEAD_BONE, MAX_BONES, ROOT_BONE};
use crate::math::{world_to_screen, ViewMatrix};
use crate::weapons::weapon_info;

const COLORKEY: COLORREF = COLORREF(0x00FF00FF);

const CT_COLOR: COLORREF = COLORREF(0x00FFB478);
const T_COLOR: COLORREF = COLORREF(0x0050C8FF);
const CT_COLOR_DIM: COLORREF = COLORREF(0x00CC8050);
const T_COLOR_DIM: COLORREF = COLORREF(0x003090CC);

const ESP_TEXT: COLORREF = COLORREF(0x00F0F0F0);
const ESP_TEXT_OUTLINE: COLORREF = COLORREF(0x00000000);

const MENU_BG: COLORREF = COLORREF(0x00281E1E);
const MENU_HEADER_BG: COLORREF = COLORREF(0x00322323);
const MENU_ACCENT: COLORREF = COLORREF(0x00FF7864);
const MENU_SELECTED_BG: COLORREF = COLORREF(0x003C2D2D);
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

/// Which parts of the ESP to draw for each player.
#[derive(Clone, Copy)]
pub struct EspParts {
    pub skeleton: bool,
    pub health: bool,
    pub weapon: bool,
    pub name: bool,
}

/// Longer names are cut so a label can't sprawl across the screen.
const MAX_NAME_CHARS: usize = 24;

pub struct Renderer {
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    font: HFONT,
    font_title: HFONT,
    font_small: HFONT,
    /// In-world text. Not antialiased: blending against the colour key would leave
    /// magenta fringes around every glyph.
    font_esp: HFONT,
    pub width: i32,
    pub height: i32,
    /// Clickable area of each menu row, in row order.
    pub menu_rows: Vec<RECT>,
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
            let font_esp = CreateFontW(
                -12, 0, 0, 0,
                400, 0, 0, 0,
                1, 0, 0, NONANTIALIASED_QUALITY.0 as u32, 0,
                w!("Tahoma"),
            );

            Self {
                mem_dc,
                bitmap,
                old_bitmap,
                font,
                font_title,
                font_small,
                font_esp,
                width,
                height,
                menu_rows: Vec::new(),
            }
        }
    }

    /// Reallocates the back buffer after the game window changes size.
    pub fn resize(&mut self, width: i32, height: i32) {
        unsafe {
            let screen_dc = GetDC(HWND::default());
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            ReleaseDC(HWND::default(), screen_dc);
            SelectObject(self.mem_dc, bitmap);
            let _ = DeleteObject(self.bitmap);
            self.bitmap = bitmap;
        }
        self.width = width;
        self.height = height;
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

    pub fn draw_player(
        &self,
        player: &PlayerData,
        vm: &ViewMatrix,
        sw: f32,
        sh: f32,
        parts: EspParts,
    ) {
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
            if parts.skeleton {
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

                if let Some((hx, hy)) = screen_bones[HEAD_BONE] {
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
            }

            if parts.health {
                if let (Some((_, head_y)), Some((_, feet_y))) =
                    (screen_bones[HEAD_BONE], screen_bones[ROOT_BONE])
                {
                    let top_y = head_y.min(feet_y) as i32 - 8;
                    let bot_y = head_y.max(feet_y) as i32 + 8;
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
            }

            if parts.weapon {
                if let (Some((name, has_mag)), Some((feet_x, _))) =
                    (weapon_info(player.weapon_id), screen_bones[ROOT_BONE])
                {
                    let bottom = screen_bones
                        .iter()
                        .filter_map(|b| b.map(|(_, y)| y as i32))
                        .max()
                        .unwrap_or(0);
                    let label = if has_mag && player.ammo >= 0 {
                        format!("{} {}", name, player.ammo)
                    } else {
                        name.to_string()
                    };
                    let old_font = SelectObject(dc, self.font_esp);
                    draw_outlined_text(dc, &label, feet_x as i32, bottom + 4, TA_TOP, ESP_TEXT);
                    SelectObject(dc, old_font);
                }
            }

            if parts.name {
                if let Some((x, _)) = screen_bones[HEAD_BONE].or(screen_bones[ROOT_BONE]) {
                    // Clear of the head circle, level with the top of the health bar.
                    let top = screen_bones
                        .iter()
                        .filter_map(|b| b.map(|(_, y)| y as i32))
                        .min()
                        .unwrap_or(0)
                        - 8;
                    let old_font = SelectObject(dc, self.font_esp);
                    draw_outlined_text(dc, &name_label(player), x as i32, top, TA_BOTTOM, ESP_TEXT);
                    SelectObject(dc, old_font);
                }
            }
        }
    }

    /// Menu rows as (label, on).
    pub fn draw_menu(&mut self, rows: &[(&str, bool)], selected: usize) {
        self.menu_rows.clear();
        let dc = self.mem_dc;

        let footer_h = 56;
        let menu_h = HEADER_H + 2 + rows.len() as i32 * ROW_H + footer_h;

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
            let mut title: Vec<u16> = "UNDERBOSS".encode_utf16().collect();
            let mut title_rc = RECT { left: MENU_X + 16, top: MENU_Y + 8, right: MENU_X + MENU_W - 16, bottom: MENU_Y + 30 };
            DrawTextW(dc, &mut title, &mut title_rc, DT_LEFT | DT_NOCLIP);

            SelectObject(dc, self.font_small);
            SetTextColor(dc, MENU_SUBTEXT);
            let mut sub: Vec<u16> = format!("v{}", crate::VERSION).encode_utf16().collect();
            let mut sub_rc = RECT { left: MENU_X + 16, top: MENU_Y + 32, right: MENU_X + MENU_W - 16, bottom: MENU_Y + HEADER_H };
            DrawTextW(dc, &mut sub, &mut sub_rc, DT_LEFT | DT_NOCLIP);

            let rows_start = MENU_Y + HEADER_H + 2;
            for (row, &(label, on)) in rows.iter().enumerate() {
                let y = rows_start + row as i32 * ROW_H;
                let row_rect = RECT { left: MENU_X, top: y, right: MENU_X + MENU_W, bottom: y + ROW_H };

                if row == selected {
                    let sel_brush = CreateSolidBrush(MENU_SELECTED_BG);
                    FillRect(dc, &row_rect, sel_brush);
                    let _ = DeleteObject(sel_brush);
                    let acc = CreateSolidBrush(MENU_ACCENT);
                    let acc_bar = RECT { right: MENU_X + 3, ..row_rect };
                    FillRect(dc, &acc_bar, acc);
                    let _ = DeleteObject(acc);
                } else if row > 0 {
                    let sep = CreateSolidBrush(COLORREF(0x00382828));
                    let sep_rect = RECT { left: MENU_X + 12, top: y, right: MENU_X + MENU_W - 12, bottom: y + 1 };
                    FillRect(dc, &sep_rect, sep);
                    let _ = DeleteObject(sep);
                }

                SelectObject(dc, self.font);
                SetTextColor(dc, MENU_TEXT);
                let mut text: Vec<u16> = label.encode_utf16().collect();
                let mut text_rc = RECT { left: MENU_X + 16, top: y + 6, right: MENU_X + 200, bottom: y + ROW_H };
                DrawTextW(dc, &mut text, &mut text_rc, DT_LEFT | DT_NOCLIP);

                draw_toggle(dc, MENU_X + MENU_W - 66, y + 7, on);
                self.menu_rows.push(row_rect);
            }

            let fy = rows_start + rows.len() as i32 * ROW_H + 8;
            SelectObject(dc, self.font_small);
            SetTextColor(dc, MENU_SUBTEXT);
            let mut nav: Vec<u16> = "UP/DOWN: Select  |  ENTER: Toggle".encode_utf16().collect();
            let mut nav_rc = RECT { left: MENU_X + 16, top: fy, right: MENU_X + MENU_W - 16, bottom: fy + 20 };
            DrawTextW(dc, &mut nav, &mut nav_rc, DT_CENTER | DT_NOCLIP);

            let mut hint: Vec<u16> = "INSERT: Toggle Menu  |  END: Exit".encode_utf16().collect();
            let mut hint_rc = RECT { left: MENU_X + 16, top: fy + 20, right: MENU_X + MENU_W - 16, bottom: fy + 40 };
            DrawTextW(dc, &mut hint, &mut hint_rc, DT_CENTER | DT_NOCLIP);

            SelectObject(dc, old_font);
        }
    }

    /// Menu row under the cursor.
    pub fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        self.menu_rows
            .iter()
            .position(|r| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
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
            let _ = DeleteObject(self.font_esp);
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

/// The player's in-game name, tagged the way the scoreboard tags bots.
fn name_label(player: &PlayerData) -> String {
    let mut name: String = player.name.chars().take(MAX_NAME_CHARS).collect();
    if player.name.chars().count() > MAX_NAME_CHARS {
        name.push_str("...");
    }
    if player.is_bot && !name.starts_with("BOT ") {
        name.insert_str(0, "BOT ");
    }
    name
}

/// Text centred on `x`, with its top or bottom (per `valign`) at `y`, outlined in
/// black so it stays readable over any background.
unsafe fn draw_outlined_text(
    dc: HDC,
    text: &str,
    x: i32,
    y: i32,
    valign: TEXT_ALIGN_OPTIONS,
    color: COLORREF,
) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    SetBkMode(dc, TRANSPARENT);
    let old_align = SetTextAlign(dc, TA_CENTER | valign);
    SetTextColor(dc, ESP_TEXT_OUTLINE);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let _ = TextOutW(dc, x + dx, y + dy, &wide);
    }
    SetTextColor(dc, color);
    let _ = TextOutW(dc, x, y, &wide);
    SetTextAlign(dc, TEXT_ALIGN_OPTIONS(old_align));
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
