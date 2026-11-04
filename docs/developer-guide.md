# Open-TmuxTabs (Rust) – Developer guide

## Overview

A native Win32 application in Rust on Microsoft's [`windows`](https://crates.io/crates/windows) crate, without a UI framework: standard controls (list box, list view, edit, button) with owner drawing for buttons and the host list, a statically linked C runtime and no runtime DLLs. Standard controls give correct text shaping and right-to-left layout (Urdu) for free.

## Layout

```
src/main.rs          entry point; `core` everywhere, `ui` only on Windows
src/core/            platform-independent logic, covered by `cargo test`
  model.rs           state of all views: hosts, selections, filters, summaries
  sshconfig.rs       host names from ~/.ssh/config (Include, patterns skipped)
  protocol.rs        remote scripts and line protocol; ssh -G parsing
  wt.rs              wt.exe command lines, batching (30 000 chars), set-up script
  display.rs         equal-width truncation of host names
  validate.rs        session names [a-zA-Z0-9_-]+
  ini.rs             OpenTmuxTabs.ini reader/writer
  po.rs, i18n.rs     gettext catalogs embedded with include_str!, language pick
src/ui/              Win32 user interface
  mod.rs             state, main window procedure, keyboard, actions, tray, hotkey
  views.rs           controls, layout and splitters, the views, owner drawing
  dialogs.rs         TaskDialog messages, explanation overlay, settings dialog
  help.rs            footer entries and their explanations
  worker.rs          ssh and wt.exe in background threads
  store.rs           settings, autostart, hotkey names
  theme.rs           colors derived from the background color
  util.rs            small helpers
locales/*.po         translations; messages.pot is the template
build.rs             icon from code, resource script (icon, manifest, version)
deploy/*.ps1         install/uninstall via irm | iex (ASCII, CRLF)
```

## Design rules

- **State lives in the model, not in the controls.** Controls are filled from `core::model::Model`; changes flow back through notifications. Views can be switched and the whole UI recreated (language switch) without losing selections or input.
- **One UI thread.** The state is a heap-allocated `State` reached through `st()`. Window procedures re-enter each other, so a reference from `st()` is never kept across a call that sends a window message; call `st()` again instead.
- **No blocking in the UI thread.** Every `ssh` and `wt.exe` call runs in a thread (`worker.rs`); results are queued and announced with `WM_APP_EVENT`. `ssh` runs with `CREATE_NO_WINDOW`, stdin closed, `ConnectTimeout=10` and a 120 s limit after which the process is killed. Every connection attempt carries a generation number; stale results are dropped.
- **Keyboard first.** The message loop calls `pre_translate` before `IsDialogMessage`: function keys and Ctrl combinations are handled on `WM_KEYDOWN`; single-letter shortcuts on `WM_CHAR`, but only while no edit control has the focus.
- **Launch errors stop that host.** If creating sessions on a host fails, no tabs are opened for that host and the error is shown.
- **Right to left.** For Urdu the main window gets `WS_EX_LAYOUTRTL`; on a language switch all controls are destroyed and recreated, so they inherit the mirroring.

## Remote protocol

One SSH call per host with a batched script: it adds an Open-TmuxTabs block (`mouse on`, `history-limit 10000`) to `~/.tmux.conf` once, creates requested sessions (`CREATED:<name>` or `EXISTS:<name>`) and lists all sessions (`tmux list-sessions -F '#{session_name}'`, or `NO_SESSIONS`). Status lines (`CFG_OK`, `CFG_DEPLOYED_RELOADED`, `CFG_DEPLOYED_PENDING`) are ignored.

Tabs are opened with `wt.exe new-tab --title "<session>@<host>" cmd.exe /c "ssh -t <host> \"tmux attach-session -t <session>\""`, several subcommands joined with ` ; `. The argument string is passed verbatim (`raw_arg`). *Set up host* passes its PowerShell script as `-EncodedCommand`, so quotes and `;` cannot interfere with wt's own parsing.

## Settings file

`OpenTmuxTabs.ini` (UTF-8) next to the EXE, fallback `%APPDATA%\OpenTmuxTabs`:

| Section | Content |
|---|---|
| `Settings` | `HotkeyMods`, `HotkeyKey`, `BackgroundColor` (`#RRGGBB`), `Language` (`auto` or a code) |
| `Window` | `X`, `Y`, `W`, `H`, `Maximized`, `HostListWidth` (logical pixels) |
| `Splits` | `host:<target>`, `open`, `direct`: splitter positions in per mille; `host:*` is the default for hosts without own values |
| `Hosts` | hosts added in the app |
| `Hidden` | hosts from the SSH config hidden by the user |
| `Usage` | `<target>=<count>,<last use as Unix time>` |

## Translations

`locales/messages.pot` is the template; `de, es, fr, eo, tlh, ur` are the catalogs. They are embedded at compile time. Two tests guard them: every msgid of the template is translated in every catalog with the same `{placeholders}`, and every text the code passes to `tr`/`trf` (and every footer explanation) is in the template. To add a text: use it in the code, add it to `messages.pot` and to every `.po`, run `cargo test`.

## Building

Windows (MSVC toolchain and Visual Studio Build Tools with the Windows SDK):

```powershell
cargo test
cargo build --release      # target\release\OpenTmuxTabs.exe
```

On Linux the core logic can be tested and the Windows code type-checked:

```sh
cargo test
rustup target add x86_64-pc-windows-msvc
cargo clippy --target x86_64-pc-windows-msvc --release -- -D warnings
```

## Release process

1. Raise `version` in `Cargo.toml` and run `cargo check` (updates `Cargo.lock`).
2. Add `## [x.y.z] – YYYY-MM-DD` to `CHANGELOG.md`; it becomes the release notes.
3. Push to `main`. The workflow tests on Linux and Windows, lints with `-D warnings`, builds the EXE, checks that no VC++ runtime DLL is imported and creates a **draft** release `v<version>` with `OpenTmuxTabs.exe` and `OpenTmuxTabs.exe.sha256` (an existing draft gets the new files).
4. Test the draft on Windows, then publish it: `gh release edit v<version> --draft=false --latest`.

## Manual test (Windows)

There are no automated UI tests. Before a release check at least: start and tray icon; hotkey shows and hides; second start; hosts from the SSH config incl. Include; add, delete and hide hosts; colors; open all, open selection (one window, per host), jump start, direct start (Ctrl+Enter), Direct session incl. a red host, Show all with sorting; filters in every list incl. toggle all/invert on filtered lists; splitters per host and view, persisted; footer overlay and F1; settings: language switch incl. Urdu (mirrored), background color light and dark, taken hotkey, start with Windows; DPI change between monitors; installer and uninstaller.
