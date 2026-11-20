use std::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetDpiForSystem() -> u32;
    fn GetThreadDpiAwarenessContext() -> *mut c_void;
    fn AreDpiAwarenessContextsEqual(a: *mut c_void, b: *mut c_void) -> i32;
}

fn main() {
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 is the pseudo-handle -4.
    let per_monitor_v2 = unsafe {
        AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), -4isize as *mut c_void) != 0
    };
    let dpi = unsafe { GetDpiForSystem() };
    println!("battery-tray skeleton, system DPI = {dpi}, per-monitor v2 = {per_monitor_v2}");
}
