//! Right-click menu, left-click action and the first-run note.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, DestroyWindow, GetCursorPos, IDYES, MB_ICONINFORMATION,
    MB_YESNO, MF_CHECKED, MF_SEPARATOR, MF_STRING, MessageBoxW, PostMessageW, SW_SHOWNORMAL,
    SetForegroundWindow, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::startup;

const ID_SETTINGS: usize = 1;
const ID_AUTOSTART: usize = 2;
const ID_EXIT: usize = 3;

fn open(uri: PCWSTR) {
    unsafe { ShellExecuteW(None, w!("open"), uri, None, None, SW_SHOWNORMAL) };
}

/// Left click: open the Windows battery settings page.
pub fn open_battery_settings() {
    open(w!("ms-settings:batterysaver"));
}

/// Right click: Open battery settings / Run at startup / Exit.
pub fn show(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let autostart = startup::autostart_enabled();
        let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, w!("Battery settings"));
        let flags = if autostart { MF_STRING | MF_CHECKED } else { MF_STRING };
        let _ = AppendMenuW(menu, flags, ID_AUTOSTART, w!("Run at startup"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Exit"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Required so the menu closes when the user clicks elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, Default::default(), Default::default());
        let _ = DestroyMenu(menu);

        match cmd.0 as usize {
            ID_SETTINGS => open_battery_settings(),
            ID_AUTOSTART => {
                let _ = startup::set_autostart(!autostart);
            }
            ID_EXIT => {
                let _ = DestroyWindow(hwnd);
            }
            _ => {}
        }
    }
}

/// Shown once: how to hide the built-in Windows battery icon (an app cannot do that itself).
pub fn first_run_note(hwnd: HWND) {
    if !startup::is_first_run() {
        return;
    }
    startup::mark_first_run_done();
    let text = HSTRING::from(
        "The new battery icon is now in the notification area.\n\n\
         To hide the built-in Windows battery icon, open Settings > Personalization > Taskbar > \
         Turn system icons on or off, and switch Power off.\n\n\
         If the new icon is not visible, drag it out of the ^ menu onto the taskbar.\n\n\
         Open the taskbar settings now?",
    );
    let answer = unsafe {
        MessageBoxW(Some(hwnd), PCWSTR(text.as_ptr()), w!("Battery tray"), MB_YESNO | MB_ICONINFORMATION)
    };
    if answer == IDYES {
        open(w!("ms-settings:taskbar"));
    }
}
