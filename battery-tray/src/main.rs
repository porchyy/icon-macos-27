#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod menu;
mod render;
mod startup;
mod tray;

use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, MSG,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetTimer, TranslateMessage,
    WINDOW_EX_STYLE, WM_DESTROY, WM_DPICHANGED, WM_LBUTTONUP, WM_POWERBROADCAST, WM_RBUTTONUP,
    WM_SETTINGCHANGE, WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{Error, Result, w};

const PBT_APMRESUMESUSPEND: usize = 0x7;
const PBT_APMPOWERSTATUSCHANGE: usize = 0xA;
const PBT_APMRESUMEAUTOMATIC: usize = 0x12;

const TIMER_QUIT: usize = 1;
const TIMER_REFRESH: usize = 2;
/// Level changes of a single percent do not always raise a power broadcast, so poll as a fallback.
const REFRESH_MS: u32 = 60_000;

/// Id of the "TaskbarCreated" message, which explorer broadcasts after it (re)starts.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

fn refresh(hwnd: HWND, why: &str) {
    if let Err(e) = tray::update(hwnd) {
        log(&format!("{why}: update failed: {e}"));
    } else {
        log(why);
    }
}

#[cfg(debug_assertions)]
fn log(msg: &str) {
    println!("{msg}");
}
#[cfg(not(debug_assertions))]
fn log(_: &str) {}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        tray::TRAY_MSG => {
            match lparam.0 as u32 & 0xffff {
                WM_LBUTTONUP => menu::open_battery_settings(),
                WM_RBUTTONUP => menu::show(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            if matches!(
                wparam.0,
                PBT_APMPOWERSTATUSCHANGE | PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND
            ) {
                refresh(hwnd, "power broadcast");
            }
            LRESULT(1) // TRUE: request granted
        }
        // Theme changes arrive here; the lParam area string is not reliable, so just re-read.
        WM_SETTINGCHANGE => {
            refresh(hwnd, "setting change");
            LRESULT(0)
        }
        WM_DPICHANGED => {
            refresh(hwnd, "dpi changed");
            LRESULT(0)
        }
        WM_TIMER => {
            match wparam.0 {
                TIMER_QUIT => unsafe { let _ = DestroyWindow(hwnd); },
                TIMER_REFRESH => refresh(hwnd, "timer"),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            tray::reset();
            refresh(hwnd, "taskbar created");
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn main() -> Result<()> {
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = w!("BatteryTrayWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(Error::from_thread());
        }
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
        // A hidden top-level window (not message-only) so it receives broadcasts.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("Battery tray"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst),
            None,
        )?;
        tray::update(hwnd)?;
        SetTimer(Some(hwnd), TIMER_REFRESH, REFRESH_MS, None);

        // `--seconds N` quits after N seconds and skips the first-run note (for smoke tests).
        let mut args = std::env::args().skip(1);
        let smoke_secs = match (args.next().as_deref(), args.next()) {
            (Some("--seconds"), Some(n)) => n.parse::<u32>().ok(),
            _ => None,
        };
        match smoke_secs {
            Some(n) => {
                SetTimer(Some(hwnd), TIMER_QUIT, n * 1000, None);
            }
            None => menu::first_run_note(hwnd),
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
