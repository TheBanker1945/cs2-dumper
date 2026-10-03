use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

mod game;
mod math;
mod mem;
mod render;

use game::{read_game_state, Offsets};
use mem::GameProcess;
use render::Renderer;

const VK_INSERT_CODE: i32 = 0x2D;
const VK_END_CODE: i32 = 0x23;

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
    let mut visible: [bool; 65] = [true; 65];
    let mut menu_open = false;
    let mut insert_was_down = false;
    let mut mouse_was_down = false;
    let mut game_hwnd = HWND::default();

    println!("[+] Overlay running!");
    println!("    INSERT = Toggle Menu");
    println!("    END    = Exit\n");

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

        let insert_is_down = key_down(VK_INSERT_CODE);
        if insert_is_down && !insert_was_down {
            menu_open = !menu_open;
            set_menu_focus(hwnd, menu_open, &mut game_hwnd);
        }
        insert_was_down = insert_is_down;

        let mouse_is_down = key_down(0x01);
        let mouse_clicked = mouse_is_down && !mouse_was_down;
        mouse_was_down = mouse_is_down;

        let (mx, my) = get_cursor_pos(hwnd);
        if menu_open {
            // In-game, CS2 clips the cursor to the screen centre; keep it released.
            unsafe {
                let _ = ClipCursor(None);
            }
            if mouse_clicked {
                handle_menu_click(&renderer, &mut visible, mx, my);
            }
        }

        let state = read_game_state(&process, &offsets, client_base, engine_base);

        renderer.begin_frame();

        for player in &state.players {
            if player.is_local {
                continue;
            }
            if !visible[player.index] {
                continue;
            }
            renderer.draw_skeleton(
                player,
                &state.view_matrix,
                state.screen_width,
                state.screen_height,
            );
        }

        if menu_open {
            renderer.draw_menu(&state.players, &visible);
            // CS2 hides the system cursor in-game, so draw our own.
            renderer.draw_cursor(mx, my);
        }

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

        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
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

/// While the menu is open the overlay must own focus: otherwise CS2 keeps the
/// cursor locked/hidden at the screen centre and every click hit-tests there.
fn set_menu_focus(hwnd: HWND, menu_open: bool, game_hwnd: &mut HWND) {
    set_click_through(hwnd, !menu_open);
    unsafe {
        if menu_open {
            let fg = GetForegroundWindow();
            if fg != hwnd {
                *game_hwnd = fg;
            }
            force_foreground(hwnd);
            let _ = ClipCursor(None);
        } else if !game_hwnd.0.is_null() && IsWindow(*game_hwnd).as_bool() {
            force_foreground(*game_hwnd);
        }
    }
}

/// SetForegroundWindow is ignored for background processes unless we attach to
/// the current foreground thread's input queue first.
unsafe fn force_foreground(target: HWND) {
    let fg = GetForegroundWindow();
    let fg_thread = GetWindowThreadProcessId(fg, None);
    let our_thread = GetCurrentThreadId();
    let attached = fg_thread != 0
        && fg_thread != our_thread
        && AttachThreadInput(our_thread, fg_thread, true).as_bool();

    let _ = BringWindowToTop(target);
    let _ = SetForegroundWindow(target);
    let _ = SetFocus(target);

    if attached {
        let _ = AttachThreadInput(our_thread, fg_thread, false);
    }
}

fn handle_menu_click(renderer: &Renderer, visible: &mut [bool; 65], mx: i32, my: i32) {
    let (l, t, r, b) = renderer.toggle_all_rect;
    if mx >= l && mx <= r && my >= t && my <= b {
        // Decide from the players shown in the menu (same as the drawn toggle state),
        // not all 64 slots — unused slots stay `true` and would make this a no-op.
        let any_on = renderer
            .toggle_areas
            .iter()
            .any(|area| visible[area.player_index]);
        for v in visible.iter_mut().skip(1).take(64) {
            *v = !any_on;
        }
        return;
    }

    for area in &renderer.toggle_areas {
        if mx >= area.left && mx <= area.right && my >= area.top && my <= area.bottom {
            visible[area.player_index] = !visible[area.player_index];
            return;
        }
    }
}

fn wait_for_key() {
    println!("\nPress Enter to exit...");
    let _ = std::io::stdin().read_line(&mut String::new());
}
