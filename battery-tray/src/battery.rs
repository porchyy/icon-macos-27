//! Battery state, read from `GetSystemPowerStatus` (kernel32).

#[repr(C)]
#[derive(Default)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: u32,
    battery_full_life_time: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
}

const FLAG_CHARGING: u8 = 0x08;
const FLAG_NO_BATTERY: u8 = 0x80;
const UNKNOWN: u8 = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryState {
    /// 0..=100
    pub level: u8,
    pub charging: bool,
}

/// Maps the raw `SYSTEM_POWER_STATUS` bytes to a state.
/// Returns `None` when there is no battery or the level is unknown.
fn from_raw(battery_flag: u8, life_percent: u8) -> Option<BatteryState> {
    if battery_flag == UNKNOWN || battery_flag & FLAG_NO_BATTERY != 0 || life_percent == UNKNOWN {
        return None;
    }
    Some(BatteryState {
        level: life_percent.min(100),
        charging: battery_flag & FLAG_CHARGING != 0,
    })
}

pub fn read() -> Option<BatteryState> {
    let mut status = SystemPowerStatus::default();
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return None;
    }
    from_raw(status.battery_flag, status.battery_life_percent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discharging() {
        // flag 1 = high, not charging
        assert_eq!(from_raw(1, 80), Some(BatteryState { level: 80, charging: false }));
    }

    #[test]
    fn charging_bit_is_detected() {
        // 1 (high) | 8 (charging)
        assert_eq!(from_raw(9, 70), Some(BatteryState { level: 70, charging: true }));
    }

    #[test]
    fn level_is_clamped_to_100() {
        assert_eq!(from_raw(1, 101).unwrap().level, 100);
    }

    #[test]
    fn unknown_level_is_none() {
        assert_eq!(from_raw(1, 255), None);
    }

    #[test]
    fn unknown_flag_is_none() {
        assert_eq!(from_raw(255, 50), None);
    }

    #[test]
    fn no_battery_is_none() {
        assert_eq!(from_raw(128, 255), None);
        assert_eq!(from_raw(128 | 1, 50), None);
    }
}
