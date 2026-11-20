# macOS 27 battery icon for Windows 10

A small Rust tray app that replaces the Windows 10 battery icon with a macOS 27 / iOS 27-style one.

## The icon

- Solid rounded body (13x10 px on a 16 px grid, radius 3), no outline, small nub on the right. Every outline edge is
  snapped to a whole pixel, so it stays crisp at 16, 20, 24 and 32 px.
- No percentage number. Level is the solid part; the rest is the same color at 50% opacity.
- Charging: a bolt is cut out of the body.
- 20% or lower (not charging): fill turns red (`#ff453a`).
- White on dark taskbars, near-black (`#1a1a1a`) on light ones.
- Optional percentage (off by default): the body gets taller and the number is cut out of it, drawn from a 3x5
  pixel digit grid. While charging below 100% a small bolt is shown before the digits.

`battery-icon.svg` is a static example (60%); `preview.html` shows all states, zoomed and at actual size.

## The app (`battery-tray/`)

- Adds a notification-area icon rendered at runtime for the current DPI.
- Updates on power changes, theme changes, DPI changes and explorer restarts, plus a 60 second fallback poll.
- Left click opens Windows battery settings. Right click: Battery settings, Show percentage, Run at startup, Exit.
  The percentage setting is saved as the DWORD `ShowPercent` under `HKCU\Software\BatteryTray`.
- On first run it explains how to hide the built-in Windows battery icon (Settings > Personalization > Taskbar >
  Turn system icons on or off > Power). An app cannot change that setting itself. If the new icon is not visible,
  drag it out of the ^ overflow menu.
- Hidden flag for smoke tests: `--seconds N` quits after N seconds and skips the first-run note.

### Build

```powershell
cd battery-tray
cargo build --release
cargo test
```

The MinGW linker cannot open paths with non-ASCII characters, so `.cargo/config.toml` sends build output to
`C:/cargo-target/battery-tray`. Change that path if you move the repo somewhere with an ASCII path. The exe is at
`C:\cargo-target\battery-tray\release\battery-tray.exe`.

### Measured (release build, Windows 10, this machine)

| What | Value |
|---|---|
| exe size | 271,360 bytes (265 KB), single file, no runtime |
| idle working set | 7.1 MB |
| idle private memory | 1.4 MB |
| threads / handles | 4 / 127 |

Startup time was not measured.

### Known limits

- At 16 px (100% scaling) the bolt is only a few pixels, and the percentage digits are small (clear at 24 px and up).
  At low levels the knocked-out digits sit on the faint red track, so contrast is low.
- The taskbar theme is read from `SystemUsesLightTheme`, which Microsoft does not document; on failure it falls back
  to light.
- Linking prints "multiple non-default manifests"; the Per-Monitor V2 manifest still takes effect (checked at
  runtime), cause not investigated.
- DPI-change handling could not be simulated from another process; theme and scaling changes also trigger a
  refresh through `WM_SETTINGCHANGE`.

Background and sources: `docs/language-research.md`.
