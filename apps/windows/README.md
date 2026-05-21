# ClipNoteX — Windows frontend (planned)

> **Status**: scaffolding only. Implementation tracked in [#TBD](https://github.com/suzuki-black/ClipNoteX/issues).

## Architecture (target)

The Windows frontend reuses the same Rust core as macOS:

```
crates/clipnotex-ffi  →  libclipnotex_ffi.lib (Windows MSVC staticlib)
                    │
                    └─►  apps/windows/  (this directory)
                          C# WinUI 3 app
                          + P/Invoke wrapper (ClipNoteXCore.cs)
```

## Build (planned)

```powershell
# 1) Build Rust staticlib for Windows
cargo build --release -p clipnotex-ffi --target x86_64-pc-windows-msvc

# 2) C# project (to be created with `dotnet new winui3 -n ClipNoteX`)
dotnet build apps\windows\ClipNoteX.sln -c Release
```

## UI components (planned)

| Component | Reference (macOS counterpart) |
|---|---|
| Notification-area icon | `StatusBarController` |
| Popup window (Win32 layered window) | `SearchPanel` (NSPanel) |
| DONE LOG window | `DoneLogWindow` |
| Preferences | `PreferencesWindow` |

The key Win32 trick mirroring macOS `NSPanel.nonactivatingPanel`:
- Create the popup with `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`
- Show with `ShowWindow(hwnd, SW_SHOWNOACTIVATE)`
- This lets the popup receive keyboard input via custom message loop
  without stealing focus from the previously-active app.

## FFI sketch (C# P/Invoke)

```csharp
public static class ClipNoteXCore {
    [DllImport("clipnotex_ffi")]
    public static extern int cnx_init([MarshalAs(UnmanagedType.LPUTF8Str)] string dataDir, int ephemeralKeys);

    [DllImport("clipnotex_ffi")]
    public static extern int cnx_paste_item([MarshalAs(UnmanagedType.LPUTF8Str)] string id, int mode);

    [DllImport("clipnotex_ffi")]
    public static extern IntPtr cnx_list_history_json([MarshalAs(UnmanagedType.LPUTF8Str)] string? query, nuint limit);

    [DllImport("clipnotex_ffi")]
    public static extern void cnx_free_string(IntPtr s);
    // ... etc
}
```

See `crates/clipnotex-ffi/include/ClipNoteX.h` for the full surface.

## Hotkeys

The `clipnotex-hotkey` crate already has a Windows backend (`global-hotkey`
on Windows uses `RegisterHotKey`). No additional Win32 work needed.

## Clipboard backend

`clipnotex-clipboard/src/windows.rs` already implements:
- Polling via `CountClipboardFormats` / `GetClipboardSequenceNumber`
- Reading via `OpenClipboard` / `GetClipboardData`
- `clipboard-win` crate handles WinRT bridging where needed

## Open questions for the Windows port

- Single-instance enforcement (`CreateMutex` named lock)
- Tray icon (Shell_NotifyIcon API or WinUI 3 NotifyIcon)
- Auto-start on login (registry / Task Scheduler)
- Toast notifications for "Captured to DONE LOG"
- DPI scaling for the popup window
