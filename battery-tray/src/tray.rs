//! Notification-area icon: renders the battery icon to an HICON and keeps it in sync.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

use windows::Win32::Foundation::{HWND, TRUE};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HGDIOBJ,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, HICON, ICONINFO, SM_CXSMICON, WM_APP,
};
use windows::core::{Error, Result, w};

use crate::{battery, render};

/// Callback message sent by the shell for mouse events on the tray icon.
pub const TRAY_MSG: u32 = WM_APP + 1;
const TRAY_ID: u32 = 1;

static ICON: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ADDED: AtomicBool = AtomicBool::new(false);
static LAST: AtomicU64 = AtomicU64::new(u64::MAX);

/// Forget what is on screen (explorer restarted and took the icon with it).
pub fn reset() {
    ADDED.store(false, Ordering::Relaxed);
    LAST.store(u64::MAX, Ordering::Relaxed);
}

/// True when the taskbar uses the light theme. The registry value is undocumented
/// (see docs/language-research.md), so any read failure falls back to light.
fn light_taskbar() -> bool {
    let mut data = 1u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    status.is_err() || data != 0
}

/// Builds an HICON from straight-alpha BGRA pixels.
unsafe fn make_icon(bgra: &[u8], size: u32) -> Result<HICON> {
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size as i32,
        biHeight: -(size as i32), // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let color = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
    unsafe { std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len()) };
    let mask = unsafe { CreateBitmap(size as i32, size as i32, 1, 1, None) };
    let icon = unsafe {
        CreateIconIndirect(&ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        })
    };
    unsafe {
        let _ = DeleteObject(HGDIOBJ(mask.0));
        let _ = DeleteObject(HGDIOBJ(color.0));
    }
    icon
}

fn tooltip(level: u8, charging: bool) -> [u16; 128] {
    let text = if charging {
        format!("Battery {level}% (charging)")
    } else {
        format!("Battery {level}%")
    };
    let mut tip = [0u16; 128];
    for (slot, unit) in tip.iter_mut().zip(text.encode_utf16().take(127)) {
        *slot = unit;
    }
    tip
}

/// Reads the current battery state and adds or refreshes the tray icon.
/// With no battery, nothing is shown.
pub fn update(hwnd: HWND) -> Result<()> {
    let Some(state) = battery::read() else {
        remove(hwnd);
        return Ok(());
    };
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.max(16) as u32;
    let light = light_taskbar();
    // Messages like WM_SETTINGCHANGE fire often; skip the redraw when nothing visible changed.
    let key = u64::from(state.level)
        | u64::from(state.charging) << 8
        | u64::from(light) << 9
        | u64::from(size) << 16;
    if ADDED.load(Ordering::Relaxed) && LAST.load(Ordering::Relaxed) == key {
        return Ok(());
    }
    let pixels = render::render(state.level, state.charging, light, size);
    let icon = unsafe { make_icon(&pixels, size)? };

    let nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE,
        uCallbackMessage: TRAY_MSG,
        hIcon: icon,
        szTip: tooltip(state.level, state.charging),
        ..Default::default()
    };
    // If our idea of "already added" is stale (e.g. explorer restarted), fall back to the other op.
    let (first, second) = if ADDED.load(Ordering::Relaxed) {
        (NIM_MODIFY, NIM_ADD)
    } else {
        (NIM_ADD, NIM_MODIFY)
    };
    let ok = unsafe { Shell_NotifyIconW(first, &nid) }.as_bool()
        || unsafe { Shell_NotifyIconW(second, &nid) }.as_bool();
    if !ok {
        unsafe { let _ = DestroyIcon(icon); }
        return Err(Error::from_thread());
    }
    ADDED.store(true, Ordering::Relaxed);
    LAST.store(key, Ordering::Relaxed);
    // The shell copies the icon, so the previous handle can be freed now.
    let old = ICON.swap(icon.0, Ordering::Relaxed);
    if !old.is_null() {
        unsafe { let _ = DestroyIcon(HICON(old)); }
    }
    Ok(())
}

/// Removes the tray icon and frees the icon handle.
pub fn remove(hwnd: HWND) {
    LAST.store(u64::MAX, Ordering::Relaxed);
    if ADDED.swap(false, Ordering::Relaxed) {
        let nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        unsafe { let _ = Shell_NotifyIconW(NIM_DELETE, &nid); }
    }
    let old = ICON.swap(std::ptr::null_mut(), Ordering::Relaxed);
    if !old.is_null() {
        unsafe { let _ = DestroyIcon(HICON(old)); }
    }
}
