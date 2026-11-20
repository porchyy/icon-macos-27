#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod render;
mod tray;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, MSG,
    PostQuitMessage, RegisterClassW, SetTimer, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY,
    WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{Error, Result, w};

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Temporary exit path until the context menu exists: right-click the icon.
        tray::TRAY_MSG => {
            if (lparam.0 as u32 & 0xffff) == WM_RBUTTONUP {
                unsafe { let _ = DestroyWindow(hwnd); }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            unsafe { let _ = DestroyWindow(hwnd); }
            LRESULT(0)
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            unsafe { PostQuitMessage(0) };
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
        // A hidden top-level window (not message-only) so it can receive broadcasts later.
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

        // `--seconds N` quits after N seconds (for smoke tests).
        let mut args = std::env::args().skip(1);
        if let (Some(flag), Some(n)) = (args.next(), args.next()) {
            if flag == "--seconds" {
                if let Ok(n) = n.parse::<u32>() {
                    SetTimer(Some(hwnd), 1, n * 1000, None);
                }
            }
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
