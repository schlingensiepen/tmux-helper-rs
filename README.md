# Open-TmuxTabs (native Windows GUI)

**Install and start** (PowerShell, no admin rights required):

```powershell
irm https://raw.githubusercontent.com/schlingensiepen/tmux-helper-rs/main/deploy/install.ps1 | iex
```

The script downloads `OpenTmuxTabs.exe` from the latest release, verifies its SHA-256 checksum, installs it to `%LOCALAPPDATA%\Programs\OpenTmuxTabs`, adds a Start menu entry and asks whether to start it right away. Running it again updates an existing installation.

---

Open-TmuxTabs is a control centre for people who keep many tmux sessions running on several Linux servers: AI agents such as Claude Code, long builds, database migrations, monitoring. It lists the hosts and their tmux sessions and opens the sessions you pick as tabs in Windows Terminal. The app stays open in the background; press **Ctrl+Alt+T** (configurable) to bring it to the front.

This is the native Windows variant, written in Rust on plain Win32: a single small EXE without runtime dependencies. A text-mode variant written in Python is available at [schlingensiepen/tmux-helper](https://github.com/schlingensiepen/tmux-helper).

## Features

- **Hosts found automatically**: every `Host` entry of `%USERPROFILE%\.ssh\config` and every host you have connected to before (`known_hosts`) appears in the list; *New host* offers the rest. Port, user and key come from the SSH config; for hosts from `known_hosts` a default user can be set.
- **Status at a glance**: blue = connecting, green = sessions loaded, red = connection failed.
- **Quick start**: Enter on *Open all detected sessions* opens every running session on every reachable host, one Windows Terminal window per host.
- **Selection**: pick sessions per host, type names of new sessions, open everything in one window or one window per host.
- **Direct start** on a host page: type new session names and press **Ctrl+Enter** to create and open them immediately.
- **Direct session**: type session names once and start them on several hosts at the same time.
- **Show all**: one sortable table of all sessions on all reachable hosts.
- **Filter above every list** (Ctrl+F or `/`), case-insensitive.
- **Everything by keyboard**; hover over a shortcut in the footer for an explanation, or press **F1** for all of them.
- **Resizable panels**: drag the splitters; sizes are remembered per host and per view.
- **Seven languages**: English, German, Spanish, French, Esperanto, Klingon and Urdu (right to left). The app starts in your Windows language and switches instantly in the settings.
- **Settings**: hotkey, background color (light or dark theme follows from it), start with Windows, language.
- **Set up host**: copies your public SSH key to a host in an interactive Windows Terminal window.

## Requirements

- Windows 10 or 11 with **Windows Terminal** (pre-installed on Windows 11, otherwise from the [Microsoft Store](https://aka.ms/terminal))
- **OpenSSH client** (Settings > Apps > Optional features; check with `ssh -V`)
- **tmux** on the remote hosts
- Key-based SSH access to the hosts (or use *Set up host* once)

## Documentation

- [User guide](docs/user-guide.md): usage, keyboard shortcuts, settings, FAQ
- [Developer guide](docs/developer-guide.md): architecture, build, release process

## Uninstall

```powershell
irm https://raw.githubusercontent.com/schlingensiepen/tmux-helper-rs/main/deploy/uninstall.ps1 | iex
```

## Project layout

```
src/core/   platform-independent logic, unit-tested on any OS
src/ui/     Win32 user interface (windows crate)
locales/    translations (gettext .po), embedded into the EXE
res/        application manifest
build.rs    generates the icon and embeds icon, manifest and version info
deploy/     install and uninstall scripts (PowerShell)
docs/       user and developer documentation
```

## License

[Apache License 2.0](LICENSE), see also [NOTICE](NOTICE)
