# Language choice for a Windows 10 custom battery indicator

Research date: 2026-11-20. Scope: a small tray/taskbar app that draws a macOS-27-style battery icon
(solid rounded body, fill = level, bolt when charging, red at <= 20%, white or near-black by taskbar theme).

Method: primary sources only (Microsoft Learn, official Rust/Cargo/Go/.NET docs). Each claim carries its
source URL. Where no primary source exists, that is stated explicitly (see "Gaps") rather than filled from
secondary write-ups. Nothing here was benchmarked; no primary source publishes startup/memory numbers for this
kind of app, so those must be measured on the target machine.

## 1. Facts that are language-independent (the Win32 layer every option sits on)

| Need | API (Microsoft Win32 docs) | Source |
|---|---|---|
| Battery state | `GetSystemPowerStatus` fills `SYSTEM_POWER_STATUS` (AC/DC, charging, life remaining, battery saver). Kernel32, min client Windows XP. | https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getsystempowerstatus |
| Power-change push | `WM_POWERBROADCAST` with `wParam = PBT_APMPOWERSTATUSCHANGE` (0xA, "Power status has changed"); `PBT_POWERSETTINGCHANGE` carries a `POWERBROADCAST_SETTING` in lParam. Needs a window with a WindowProc. | https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast |
| Tray icon | `Shell_NotifyIcon` with `NIM_ADD` / `NIM_MODIFY` / `NIM_DELETE`; icon passed as `HICON` in `NOTIFYICONDATA.hIcon`; identify by `hWnd`+`uID` or `guidItem` (Windows 7+). | https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyicona , https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw |
| Tray icon size / DPI | Docs: a lone 16x16 icon is scaled up on high-DPI systems with "an unattractive result"; provide 16x16 and 32x32 and load with `LoadIconMetric`. A runtime-generated HICON has no resource, so the app must render at the small-icon size for the current DPI and re-render on DPI change. | https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw |
| DPI awareness | Desktop apps are DPI-unaware (bitmap-stretched) by default; declare Per-Monitor V2 (Windows 10 1703+, via application manifest); handle `WM_DPICHANGED`; use `GetDpiForWindow` / `GetSystemMetricsForDpi`. The doc's framework table lists GDI and GDI+ as "None" (app does the scaling) and Win32/WinForms/WPF as needing developer work. | https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows |
| Registry change wait | `RegNotifyChangeKeyValue` detects a single change (re-arm after each); must run on a persistent thread unless `REG_NOTIFY_THREAD_AGNOSTIC` (Windows 8+); re-arming with the same parameters before the previous wait completes leaks. | https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regnotifychangekeyvalue |
| Settings broadcast | `WM_SETTINGCHANGE` goes to all top-level windows; lParam is a string for the changed area and "does not usually indicate which specific system parameter changed", so re-read what you use. | https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange |

Consequence: one hidden top-level window (for `WM_POWERBROADCAST`, `WM_SETTINGCHANGE`, `WM_DPICHANGED`, tray
callback) plus one icon-rendering function is the whole app. Every language below can do that; they differ in
how much glue surrounds it.

### Theme detection: what Microsoft documents and what it does not

- The registry values `SystemUsesLightTheme` (taskbar/system) and `AppsUseLightTheme` under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` are NOT documented on any Microsoft Learn
  Win32 page I could find, and I could not open a Microsoft-owned source file using them. The only related Learn
  page is the unattend setting `UWPAppsUseLightTheme`:
  https://learn.microsoft.com/en-us/windows-hardware/customize/desktop/unattend/microsoft-windows-shell-setup-themes-uwpappsuselighttheme
  Treat the value names as an undocumented contract (see Gaps).
- Documented alternative: WinRT `UISettings.GetColorValue(UIColorType)` and the `UISettings.ColorValuesChanged`
  event ("Occurs when color values have changed"):
  https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.colorvalueschanged ,
  https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.getcolorvalue .
  Caveat: this reflects the app color mode, not specifically the taskbar mode (the `SystemUsesLightTheme` vs
  `AppsUseLightTheme` split matters for this app), and the fetched page does not address use from Win32 desktop apps.
- Change notification, both documented generically: `RegNotifyChangeKeyValue` on the Personalize key (above) or
  `WM_SETTINGCHANGE`. Re-read the value on every `WM_SETTINGCHANGE`.

## 2. Per-language findings

### 2.1 Rust (`windows` crate; the previous implementation)

1. Icon + DPI: direct Win32 via generated bindings. The docs.rs page for the crate is a stub that points to the
   Rust-for-Windows docs (https://docs.rs/windows/latest/windows/), so API behaviour is the Microsoft behaviour in
   section 1. Draw with GDI (`CreateDIBSection` -> `CreateIconIndirect`), GDI+ or Direct2D through the same crate.
   No in-crate tray abstraction: write the message loop and `Shell_NotifyIcon` calls.
2. Battery: plain Win32 calls (`GetSystemPowerStatus`, `WM_POWERBROADCAST`); window class and WndProc by hand.
3. Theme: `RegOpenKeyEx`/`RegQueryValueEx` + `RegNotifyChangeKeyValue`.
4. Size / startup / memory / runtime: Cargo documents the size levers `opt-level = "s"|"z"`, `lto`,
   `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`, and warns "s"/"z" are "not necessarily smaller",
   so measure: https://doc.rust-lang.org/cargo/reference/profiles.html . No managed runtime. No primary source
   gives startup or RSS numbers. Static-CRT linking was not verified from a primary source.
5. Build/distribution: `cargo build --release` gives one .exe; MSVC (or GNU) toolchain on the build machine only.
   The DPI manifest must be embedded by you (build script/resource); Per-Monitor V2 is requested through the
   application manifest per the High-DPI page above.
6. Taskbar placement: see section 3 (same for all languages). A COM deskband is possible through the crate's COM
   bindings but needs hand-written COM objects.

### 2.2 C# on .NET (WinForms / WPF / WinUI)

1. Icon + DPI: `System.Windows.Forms.NotifyIcon` wraps the notification area; `Icon` sets the image and `Visible`
   must be true: https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.notifyicon . A runtime-drawn icon
   means rendering a GDI+ `Bitmap` and converting its HICON to an `Icon`; that HICON is unmanaged and needs manual
   `DestroyIcon` (standard Win32 ownership; the NotifyIcon page does not discuss it, so verify in code). DPI: the
   High-DPI page rates WinForms "Limited automatic per-monitor DPI scaling for some controls" and says most desktop
   UI frameworks require developers to resize contents themselves
   (https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows), so
   choosing the icon size for the current DPI is still your job. No first-party WPF/WinUI tray control was found in
   the pages checked; the common route is WinForms `NotifyIcon` or raw P/Invoke.
2. Battery: `SystemEvents.PowerModeChanged` is documented as "Occurs when the user suspends or resumes the
   system"; it is not a battery-level event, only fires while a message pump runs, and static handlers must be
   detached or they leak: https://learn.microsoft.com/en-us/dotnet/api/microsoft.win32.systemevents.powermodechanged .
   Level/charging changes therefore still need `GetSystemPowerStatus` (P/Invoke) triggered by `WM_POWERBROADCAST`
   through a WndProc override. `SystemInformation.PowerStatus` was not fetched (unverified).
3. Theme: registry read via `Microsoft.Win32.Registry`; `SystemEvents.UserPreferenceChanged` not fetched
   (unverified); or the `WM_SETTINGCHANGE` path. WinRT `UISettings` is reachable from .NET (app-color-mode only).
4. Size / startup / memory / runtime (Microsoft documents the model, not numbers):
   - Framework-dependent: "Small deployment" but "Requires pre-installing the runtime";
     self-contained: no preinstall but "Larger deployments"; single-file: "Slower startup" (extraction at runtime);
     ReadyToRun: better startup, larger size: https://learn.microsoft.com/en-us/dotnet/core/deploying/
   - Native AOT: "faster startup time and smaller memory footprints", runs without the .NET runtime installed;
     limitations include no dynamic loading, no `Reflection.Emit`, "Windows: No built-in COM", required trimming:
     https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/ . That page does not list WinForms or WPF as
     supported and I found no primary statement that `NotifyIcon` works under Native AOT; assume it does not until
     verified. A Native AOT tray app would mean raw P/Invoke with no WinForms.
   - Inbox .NET Framework version on Windows 10 was not verified from a primary source.
5. Build/distribution: `dotnet publish` with the flags above is the easiest toolchain. Native AOT on Windows needs the
   Visual Studio "Desktop development with C++" workload (Native AOT page above).
6. Taskbar placement: no managed API; same limits as section 3. A managed deskband would load the CLR into
   explorer.exe (my inference; see Gaps).

### 2.3 C++ (raw Win32)

1. Icon + DPI: most direct. `NOTIFYICONDATA`, `LoadIconMetric` (Learn gives a code sample on the NOTIFYICONDATA page),
   `CreateIconIndirect`, GDI+/Direct2D, `GetDpiForWindow`, `WM_DPICHANGED` (sample linked from the High-DPI page).
2. Battery: `GetSystemPowerStatus` and `WM_POWERBROADCAST` with no wrapper.
3. Theme: `RegNotifyChangeKeyValue` has a full C++ example on its Learn page.
4. Size / startup / memory / runtime: no managed runtime; single .exe possible with static CRT. No primary source
   fetched quantifies size.
5. Build: MSVC toolchain; most boilerplate (window class, message loop, manifest, resources).
6. Taskbar placement: the only language where the documented deskband route has an official walkthrough (section 3).

### 2.4 Go and Python

Go
- gc produces "statically-linked binaries by default" and "all Go binaries therefore include the Go runtime"; a
  trivial program is a couple of MB, and `-ldflags=-w` drops DWARF info: https://go.dev/doc/faq . No runtime install.
- `golang.org/x/sys/windows` lists `RegNotifyChangeKeyValue`, `RegOpenKeyEx`, `NewLazySystemDLL`, `NewCallback`:
  https://pkg.go.dev/golang.org/x/sys/windows . In the part of that page I could read (first ~100k of ~520k
  characters) `Shell_NotifyIcon` and `GetSystemPowerStatus` were absent, so they likely need `NewLazySystemDLL`
  bindings with `NewCallback` for the WndProc (unconfirmed for the unread part). Tray, DPI and GDI are hand-bound,
  like Rust but without typed, Microsoft-generated bindings.
- Taskbar placement: section 3.

Python
- `ctypes` is stdlib, but I did not fetch Python packaging docs and there is no first-party single-exe story I could
  cite. A frozen build bundles the interpreter or requires Python installed. Tray support needs a third-party
  library (excluded as non-primary) or ctypes boilerplate like Go. Judged a poor fit for an always-on resident
  widget; this is judgment, not a measured or cited figure.

## 3. Placing a widget on the taskbar itself (not just the notification area)

This is language-independent: the risk comes from the shell.

1. Notification area (supported): `Shell_NotifyIcon`, fully documented (section 1). I found no documented parameter
   that forces an icon to stay out of the overflow flyout (absence unverified).
2. Deskband (documented but legacy): Learn describes desk bands as COM band objects the user turns on from the
   taskbar's Toolbars submenu; implement `IDeskBand`, `IObjectWithSite`, `IPersistStream`, register under
   `CATID_DeskBand`, DLL as an in-proc server, window created as a child of the site's window:
   https://learn.microsoft.com/en-us/windows/win32/shell/band-objects . That page (updated 2025-03) carries no
   deprecation notice. The Windows client "Deprecated features" list (updated 2026-11-11) has no deskband or
   taskbar-toolbar entry (only "Taskbar settings roaming"):
   https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features . Absence of a notice is not a support
   guarantee. That Windows 11 dropped taskbar toolbars/deskbands appeared only in community answers and secondary
   pages in my searches; no Microsoft-owned statement was found, so it is NOT claimed as fact. Risks: a registered
   DLL (regsvr32 / COM registration), code running inside explorer.exe (a crash takes the shell down), user-resized
   toolbar slot instead of free space, and a managed implementation would host the CLR in explorer.
3. Window parented to the taskbar (`Shell_TrayWnd` + `SetParent`): no Microsoft documentation of the taskbar's window
   hierarchy as an extension point. The High-DPI page's table shows `SetParent` across DPI-awareness modes can fail
   (`ERROR_INVALID_STATE`) or force-reset the process, and cross-process `SetParent` is a "Forced reset" case:
   https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows .
   Depends on undocumented shell internals that any Windows update can change.
4. Injected overlay (hooking/injecting into explorer.exe): undocumented; no primary source supports it; likely to be
   flagged by security software. Not recommended.
5. Topmost window positioned over the taskbar rectangle: uses only documented window APIs, but fights the shell for
   z-order and breaks with auto-hide, multiple monitors and DPI changes. Not verified against a primary page here;
   the lowest-risk way to look "on the taskbar".

## 4. Comparison (only claims backed above)

| Criterion | Rust | C# (.NET) | C++ Win32 | Go | Python |
|---|---|---|---|---|---|
| Tray icon API | raw via `windows` crate | `NotifyIcon` (WinForms) | native | hand-bound | hand-bound / 3rd-party |
| Runtime-drawn HICON | GDI/D2D via crate | GDI+ Bitmap -> Icon, manual HICON free | native | hand-bound GDI | hand-bound GDI |
| Power events | WndProc by hand | WndProc override; `PowerModeChanged` is suspend/resume only | WndProc | WndProc via `NewCallback` | ctypes callback |
| Theme detection | registry + notify | registry + notify | registry + notify (Learn sample) | `RegNotifyChangeKeyValue` in x/sys | ctypes |
| Runtime needed | none | .NET runtime unless self-contained/AOT | none | none (runtime inside exe) | interpreter bundled or installed |
| Documented size levers | Cargo profiles | trimming / AOT / single-file | n/a | `-ldflags=-w` | n/a |
| Deskband feasibility | hand-written COM | fragile (CLR in explorer) | official walkthrough | impractical (COM) | impractical |

## 5. Gaps (not closable with primary sources)

- `SystemUsesLightTheme` / `AppsUseLightTheme` semantics: not on Learn; guard it (default to light on read failure).
- `ImmersiveColorSet` as lParam of theme-change `WM_SETTINGCHANGE`: not in the Learn page; re-read on every message.
- `TaskbarCreated` re-add after explorer restart, `SystemInformation.PowerStatus`, `UserPreferenceChanged`, inbox
  .NET Framework version on Windows 10, static CRT for Rust: not fetched.
- Startup time and memory per language: no primary source; measure the real app.
- Windows 11 deskband removal: secondary sources only.
- Go `x/sys/windows` coverage: only the first 100k characters of the page were read.
- windows-rs README returned 404 at the URL tried, so crate-feature and size statements are not cited.

## 6. Recommendation

Stay with Rust (`windows` crate) unless iteration speed matters more than footprint; C++ Win32 is the equal
alternative; ship the notification-area icon rather than a true taskbar widget.

- Every option calls the same small Win32 surface (hidden window, `Shell_NotifyIcon`, `GetSystemPowerStatus`,
  `WM_POWERBROADCAST`, one registry watch, one bitmap-to-HICON function), so switching language buys no capability.
- Rust: one .exe, no runtime, documented size profile (`opt-level="z"`, `lto`, `panic="abort"`, `strip`); cost is
  hand-written message loop and manifest.
- C#: fastest to write (`NotifyIcon`), but still needs P/Invoke for HICON lifetime, power events and DPI sizing, and
  needs the .NET runtime or a larger self-contained bundle; Native AOT removes the runtime but WinForms support is
  not confirmed.
- Go/Python: no advantage; hand-bound Win32 plus a larger runtime or interpreter.
- Taskbar-embedded widget: deskband is the only documented mechanism (COM DLL inside explorer.exe; crash blast
  radius is the shell); `SetParent` into the taskbar and injection are undocumented and fragile.

Main trade-offs: Rust = smallest footprint and memory safety but most boilerplate; C# = least code but runtime/size
overhead and AOT limits; C++ = least abstraction but no memory safety; taskbar embedding = legacy documented deskband
vs undocumented hacks that can break with any Windows update.
