//! Per-user registry helpers: run-at-startup and the first-run flag.

use std::ffi::c_void;

use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_ANY, RRF_RT_REG_DWORD, RegDeleteKeyValueW,
    RegGetValueW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PCWSTR, Result};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_KEY: &str = r"Software\BatteryTray";
const APP_NAME: &str = "BatteryTray";
const FIRST_RUN_VALUE: &str = "FirstRunDone";
const SHOW_PERCENT_VALUE: &str = "ShowPercent";

fn value_exists(subkey: &str, name: &str) -> bool {
    let (subkey, name) = (HSTRING::from(subkey), HSTRING::from(name));
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_ANY,
            None,
            None,
            None,
        )
    }
    .is_ok()
}

fn set_string(subkey: &str, name: &str, value: &str) -> Result<()> {
    let (subkey, name) = (HSTRING::from(subkey), HSTRING::from(name));
    let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr() as *const c_void),
            (data.len() * 2) as u32,
        )
    }
    .ok()
}

fn delete_value(subkey: &str, name: &str) -> Result<()> {
    if !value_exists(subkey, name) {
        return Ok(());
    }
    let (subkey, name) = (HSTRING::from(subkey), HSTRING::from(name));
    unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), PCWSTR(name.as_ptr())) }.ok()
}

pub fn autostart_enabled() -> bool {
    value_exists(RUN_KEY, APP_NAME)
}

pub fn set_autostart(on: bool) -> Result<()> {
    if on {
        let exe = std::env::current_exe().map_err(|e| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string()))?;
        set_string(RUN_KEY, APP_NAME, &format!("\"{}\"", exe.display()))
    } else {
        delete_value(RUN_KEY, APP_NAME)
    }
}

fn get_dword(subkey: &str, name: &str) -> Option<u32> {
    let (subkey, name) = (HSTRING::from(subkey), HSTRING::from(name));
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    }
    .is_ok()
    .then_some(data)
}

fn set_dword(subkey: &str, name: &str, value: u32) -> Result<()> {
    let (subkey, name) = (HSTRING::from(subkey), HSTRING::from(name));
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_DWORD.0,
            Some(&value as *const u32 as *const c_void),
            size_of::<u32>() as u32,
        )
    }
    .ok()
}

/// DWORD 0/1, off by default. (An earlier version of this app used the same value.)
pub fn show_percent() -> bool {
    get_dword(APP_KEY, SHOW_PERCENT_VALUE).is_some_and(|v| v != 0)
}

pub fn set_show_percent(on: bool) -> Result<()> {
    set_dword(APP_KEY, SHOW_PERCENT_VALUE, u32::from(on))
}

pub fn is_first_run() -> bool {
    !value_exists(APP_KEY, FIRST_RUN_VALUE)
}

pub fn mark_first_run_done() {
    let _ = set_string(APP_KEY, FIRST_RUN_VALUE, "1");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_exists_delete_roundtrip() {
        let key = r"Software\BatteryTrayTest";
        let _ = delete_value(key, "v");
        assert!(!value_exists(key, "v"));
        set_string(key, "v", "hello").unwrap();
        assert!(value_exists(key, "v"));
        delete_value(key, "v").unwrap();
        assert!(!value_exists(key, "v"));
        // deleting a missing value is fine
        delete_value(key, "v").unwrap();
    }

    #[test]
    fn dword_roundtrip() {
        let key = r"Software\BatteryTrayTest";
        let _ = delete_value(key, "d");
        assert_eq!(get_dword(key, "d"), None);
        set_dword(key, "d", 1).unwrap();
        assert_eq!(get_dword(key, "d"), Some(1));
        set_dword(key, "d", 0).unwrap();
        assert_eq!(get_dword(key, "d"), Some(0));
    }
}
