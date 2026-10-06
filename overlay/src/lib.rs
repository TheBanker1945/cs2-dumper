use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

mod aim;
mod game;
mod math;
mod mem;
mod render;
mod smoke;
mod trigger;
mod visibility;
mod weapons;

use aim::AimAssist;
use game::{read_game_state, Offsets};
use mem::GameProcess;
use render::{EspParts, Renderer};
use smoke::SmokeScanner;
use trigger::TriggerBot;
use visibility::Visibility;

const VK_INSERT_CODE: i32 = 0x2D;
const VK_END_CODE: i32 = 0x23;

/// Menu rows and their state at startup. Every row works on its own.
const MENU_ROWS: [(&str, bool); 5] = [
    ("Skeleton", true),
    ("Health Bar", true),
    ("Weapon", true),
    ("Name", true),
    ("Trigger Bot (hold Mouse 4)", false),
];
const ROW_SKELETON: usize = 0;
const ROW_HEALTH: usize = 1;
const ROW_WEAPON: usize = 2;
const ROW_NAME: usize = 3;
const ROW_TRIGGER: usize = 4;

pub fn run() {
    println!("=== CS2 Skeleton ESP Overlay ===");
    println!("    Educational Purpose Only\n");

    println!("[*] Loading offsets from output/ ...");
    let offsets = match Offsets::load("output") {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[!] {}", e);
            eprintln!("[!] Make sure to run the cs2-dumper first.");
            wait_for_key();
            return;
        }
    };
    println!("[+] Offsets loaded successfully.");

    println!("[*] Searching for CS2 process...");
    let process = match GameProcess::open("cs2.exe") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[!] {}", e);
            eprintln!("[!] Run this as Administrator if CS2 is running.");
            wait_for_key();
            return;
        }
    };

    let client_base = match process.module_base("client.dll") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[!] {}", e);
            wait_for_key();
            return;
        }
    };
    let engine_base = match process.module_base("engine2.dll") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[!] {}", e);
            wait_for_key();
            return;
        }
    };
    println!("[+] CS2 found. client.dll @ {:#X}, engine2.dll @ {:#X}", client_base, engine_base);

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let hwnd = create_overlay_window(screen_w, screen_h);
    if hwnd.0.is_null() {
        eprintln!("[!] Failed to create overlay window.");
        wait_for_key();
        return;
    }

    let mut renderer = Renderer::new(screen_w, screen_h);
    let mut toggles = MENU_ROWS.map(|(_, on)| on);
    let mut trigger = TriggerBot::new();
    let mut aim = AimAssist::new();
    let mut visibility = Visibility::new();
    let mut smoke_scanner = SmokeScanner::new();
    let mut menu_open = false;
    let mut selected: usize = 0;
    let mut insert_key = KeyEdge::new(VK_INSERT_CODE);
    let mut mouse_key = KeyEdge::new(0x01);
    let mut up_key = KeyEdge::new(VK_UP.0 as i32);
    let mut down_key = KeyEdge::new(VK_DOWN.0 as i32);
    let mut left_key = KeyEdge::new(VK_LEFT.0 as i32);
    let mut right_key = KeyEdge::new(VK_RIGHT.0 as i32);
    let mut enter_key = KeyEdge::new(VK_RETURN.0 as i32);

    println!("[+] Overlay running!");
    println!("    INSERT          = Toggle Menu");
    println!("    UP / DOWN       = Select row");
    println!("    ENTER / <- / -> = Toggle selected row");
    println!("    END             = Exit");
    println!("    Trigger Bot is off at startup. Hold Mouse 4 to snap to the head of an enemy");
    println!("    in clear view and fire; shooting by hand also pulls onto the head.\n");

    loop {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if key_down(VK_END_CODE) {
            println!("[*] Exiting...");
            break;
        }

        if insert_key.pressed() {
            menu_open = !menu_open;
            set_click_through(hwnd, !menu_open);
        }

        // Poll every key each frame so edge state stays current even while the menu is closed.
        let clicked = mouse_key.pressed();
        let up = up_key.pressed();
        let down = down_key.pressed();
        let toggle = enter_key.pressed() | left_key.pressed() | right_key.pressed();

        if menu_open {
            // Keyboard navigation works in every display mode. In a match CS2 owns the
            // cursor (hidden and pinned to the screen centre), so mouse clicks can't
            // reach the menu there.
            let row_count = MENU_ROWS.len();
            if up {
                selected = (selected + row_count - 1) % row_count;
            }
            if down {
                selected = (selected + 1) % row_count;
            }
            if toggle {
                toggles[selected] = !toggles[selected];
            }

            if clicked {
                let (mx, my) = get_cursor_pos(hwnd);
                if let Some(row) = renderer.row_at(mx, my) {
                    selected = row;
                    toggles[row] = !toggles[row];
                }
            }
        }

        let state = read_game_state(&process, &offsets, client_base, engine_base, &mut smoke_scanner);
        visibility.update(&state, Instant::now());

        // Head aim is part of the trigger bot: holding Mouse 4 pulls onto the head and
        // fires once the crosshair is there, and shooting by hand pulls onto it too.
        let trigger_on = toggles[ROW_TRIGGER] && !menu_open && game_focused(process.pid());
        let trigger_key = trigger_on && key_down(VK_XBUTTON1.0 as i32);
        let aiming = trigger_key || (trigger_on && key_down(VK_LBUTTON.0 as i32));
        aim.update(aiming, &state, &visibility);
        trigger.update(trigger_key && aim.lined_up(), &state, &visibility);

        renderer.begin_frame();

        let parts = EspParts {
            skeleton: toggles[ROW_SKELETON],
            health: toggles[ROW_HEALTH],
            weapon: toggles[ROW_WEAPON],
            name: toggles[ROW_NAME],
        };
        for player in state.players.iter().filter(|p| !p.is_local) {
            renderer.draw_player(
                player,
                &state.view_matrix,
                state.screen_width,
                state.screen_height,
                parts,
            );
        }

        if menu_open {
            let rows: Vec<(&str, bool)> = MENU_ROWS
                .iter()
                .zip(toggles)
                .map(|(&(label, _), on)| (label, on))
                .collect();
            renderer.draw_menu(&rows, selected);
        }

        keep_above_game(hwnd, process.pid());
        renderer.end_frame(hwnd);

        std::thread::sleep(Duration::from_millis(16));
    }
}

fn create_overlay_window(width: i32, height: i32) -> HWND {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("CS2OverlayClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: CreateSolidBrush(COLORREF(0x00FF00FF)),
            ..Default::default()
        };

        RegisterClassExW(&wc);

        // NOACTIVATE: clicking the menu must never take focus from CS2 — in exclusive
        // Fullscreen the game minimizes as soon as it loses focus.
        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!(""),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            width,
            height,
            None,
            None,
            wc.hInstance,
            None,
        ) {
            Ok(h) => h,
            Err(_) => return HWND::default(),
        };

        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0x00FF00FF), 0, LWA_COLORKEY);
        let _ = ShowWindow(hwnd, SW_SHOW);

        hwnd
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn key_down(vk: i32) -> bool {
    unsafe { GetAsyncKeyState(vk) < 0 }
}

fn get_cursor_pos(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = ScreenToClient(hwnd, &mut pt);
        (pt.x, pt.y)
    }
}

/// Injected clicks go to whichever window has focus, so only fire while it's CS2.
fn game_focused(game_pid: u32) -> bool {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
        pid == game_pid
    }
}

/// CS2 (an SDL window) makes itself topmost in Fullscreen mode and re-raises itself
/// every time it gains focus, which puts it above the overlay. Climb back to the top
/// of the topmost band whenever one of the game's windows is above ours.
fn keep_above_game(hwnd: HWND, game_pid: u32) {
    unsafe {
        let mut above = GetWindow(hwnd, GW_HWNDPREV);
        while let Ok(w) = above {
            let mut pid = 0;
            GetWindowThreadProcessId(w, Some(&mut pid));
            if pid == game_pid && IsWindowVisible(w).as_bool() {
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                return;
            }
            above = GetWindow(w, GW_HWNDPREV);
        }
    }
}

fn set_click_through(hwnd: HWND, click_through: bool) {
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let new_style = if click_through {
            style | WS_EX_TRANSPARENT.0
        } else {
            style & !WS_EX_TRANSPARENT.0
        };
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_style as i32);
        let _ = SetWindowPos(
            hwnd,
            HWND::default(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Rising-edge detector over GetAsyncKeyState's reliable "is down" bit.
struct KeyEdge {
    vk: i32,
    was_down: bool,
}

impl KeyEdge {
    fn new(vk: i32) -> Self {
        Self { vk, was_down: false }
    }

    fn pressed(&mut self) -> bool {
        let down = key_down(self.vk);
        let pressed = down && !self.was_down;
        self.was_down = down;
        pressed
    }
}

fn wait_for_key() {
    println!("\nPress Enter to exit...");
    let _ = std::io::stdin().read_line(&mut String::new());
}
