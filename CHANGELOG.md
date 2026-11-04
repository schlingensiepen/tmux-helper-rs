# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org/).
A draft release is created automatically when the `version` in `Cargo.toml` on `main` has no release yet; it is published by hand.

## [1.0.0] – 2026-11-04

### Added
- Native Windows GUI (Rust, Win32) with the functions of the text-mode Open-TmuxTabs: host list with status colors, session selection, new sessions, quick start, open selection in one window or per host, Direct session, Show all with sorting, Set up host.
- Hosts are read from `%USERPROFILE%\.ssh\config` (including `Include`); additional hosts can be added by hand. Deleting a config host only hides it.
- Filter field above every list (Ctrl+F or `/`), case-insensitive.
- Direct start on a host page: button and Ctrl+Enter create and open the typed sessions at once.
- Resizable panels; splitter positions are stored per host and per view.
- Footer with the shortcuts of the current view; hovering shows an explanation, F1 shows all.
- Global hotkey (default Ctrl+Alt+T), tray icon, single instance, start with Windows.
- Settings dialog: hotkey, background color with derived light/dark theme, start with Windows, language.
- Languages: English, German, Spanish, French, Esperanto, Klingon, Urdu (mirrored layout); live switching.
- Installer and uninstaller (`irm … | iex`), Start menu entry.
