# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org/).
A draft release is created automatically when the `version` in `Cargo.toml` on `main` has no release yet; it is published by hand.

## [1.0.1] – 2026-11-05

### Added
- Hosts from `known_hosts` (and `known_hosts2`) appear in the host list automatically, connected as the new setting *Default user for known hosts* (empty = your Windows user). Names that share a server key count as one machine; hashed entries, IP-only entries and ports other than 22 are skipped.
- *New host* offers a filterable list of known hosts that are not in the list yet and of all hidden hosts. Enter or double-click copies one into the input field; hidden hosts come back this way.
- Git hosting services (github.com, gitlab.com, bitbucket.org, Azure DevOps, codeberg.org) are hidden by default; they can be taken back from the list.

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
