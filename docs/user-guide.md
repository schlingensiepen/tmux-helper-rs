# Open-TmuxTabs – User guide

## Installation

Open PowerShell and enter:

```powershell
irm https://raw.githubusercontent.com/schlingensiepen/tmux-helper-rs/main/deploy/install.ps1 | iex
```

The script downloads `OpenTmuxTabs.exe` from the latest release, verifies the SHA-256 checksum, places it in `%LOCALAPPDATA%\Programs\OpenTmuxTabs`, adds a Start menu entry, shows the configured hotkey and asks *Start Open-TmuxTabs now?* (Enter or Y = yes). If yes, the app starts with the settings dialog open. No admin rights are required. Running the command again updates the app. A specific version: set `$env:OPENTMUXTABS_VERSION = 'v1.0.0'` first.

Alternatively download `OpenTmuxTabs.exe` from the releases page and run it from any folder. Windows SmartScreen may warn because the file is not signed: *More info → Run anyway*.

## Before you start

- **Windows Terminal** and the **OpenSSH client** must be installed.
- **tmux** must be installed on the remote hosts.
- SSH login must work without a password prompt (key-based). Test it with `ssh <host>` in a terminal; confirm the host fingerprint once with `yes`. For a host without a key, use **Set up host** in the app.

## The window

```
┌──────────────────────────────────────────────────────────┐
│ Open-TmuxTabs                              [en] English  │
├──────────────────┬───────────────────────────────────────┤
│ [Filter…       ] │                                       │
│ + New host       │   detail area: changes with the       │
│ ──────────────── │   entry selected on the left          │
│ root@dev-server  │                                       │
│ clawbook         │                                       │
│ >> Open          │                                       │
│ ──────────────── │                                       │
│ Direct session   │                                       │
│ Show all         │                                       │
├──────────────────┴───────────────────────────────────────┤
│ q Quit  n New host  o Open  x Open all  + Direct …       │
└──────────────────────────────────────────────────────────┘
```

**Left: the host list.** At the top *+ New host*, then your hosts, then *>> Open*; below a separator the two host-independent views *Direct session* and *Show all*. Moving through the list with the arrow keys switches the right side at once; Enter or Tab moves the focus to the right side.

Host colors: **blue** = connecting, **green** = connected, the sessions are listed, **red** = connection failed.

**Where the hosts come from.**

- Every `Host` entry of `%USERPROFILE%\.ssh\config` (entries with wildcards such as `Host *` are skipped; `Include` files are read too). Port, user and key are taken from the SSH config.
- Every host you have connected to before, from `%USERPROFILE%\.ssh\known_hosts` (and `known_hosts2`). They are connected as `<default user>@<name>`; set the default user in the settings (empty = your Windows user name, as with plain `ssh`). Names that belong to the same server (same server key) appear once. Hashed entries, IP-only entries and ports other than 22 are not added automatically. Git hosting services such as github.com are hidden from the start.
- Hosts you add with *New host*; the app stores them.

Deleting a host from the SSH config or from `known_hosts` only hides it in the app; the files are not changed. *New host* lists below the input field all known hosts that are not in the list and all hidden hosts: Enter or double-click copies one into the input field (the cursor is at the start, so you can change the user), Enter adds it. **F5** reads the SSH config and `known_hosts` again.

**Footer.** The shortcuts of the current view. Point at one with the mouse for an explanation; click to run it; **F1** shows all explanations.

**Splitters.** The border between the host list and the detail area and the borders inside a view can be dragged with the mouse (or moved with **Ctrl+Shift+arrow keys**). The width of the host list is the same everywhere; the inner sizes are remembered per host and per view.

**Filters.** Every list has a filter field above it. **Ctrl+F** or **/** jumps into the filter of the current list; typing narrows the list (upper/lower case does not matter); **Esc** or **Arrow down** returns to the list. In the host list the filter only affects hosts; the fixed entries stay. *Toggle all* and *Invert* only affect the entries that are visible.

## Views

**Open** (start view). The button *Open all detected sessions* is focused at start: Enter opens every running session on every reachable host, one Windows Terminal window per host (hosts that are still connecting are waited for). Below, the summary of everything you selected on the host pages, with *One WT – Open selection* (`e`, one window) and *Per host – Open selection* (`p`, one window per host). Afterwards the selections and typed names are cleared.

**Host – connected.** The running sessions with checkboxes, a filter above them, *Reload* (`r`) and *Delete*. Below: a field for new session names (one per line, only letters, digits, `_` and `-`) and the button **Direct start** (**Ctrl+Enter**): it creates the typed sessions on this host and opens them at once in one Windows Terminal window. Names that exist already are simply attached.

**Host – failed.** The SSH error message, *Reconnect* (`r`), *Set up host* (`s`: opens a Windows Terminal window that copies your public key `%USERPROFILE%\.ssh\id_*.pub` to the host; type the password once) and *Delete*.

**Host – connecting.** Shown while the app connects.

**Direct session.** Pick target hosts (red hosts can be picked; they are reconnected first), type session names, check the summary (*existing* = attach, *new* = create), then *One WT – Execute* (`j`) or *Per host – Execute* (`p`). Selection and names stay, so you can run the same set again.

**Show all.** One table with the sessions of all connected hosts. Sort with a click on a column header or with `h` (host) and `s` (session); pressing again reverses the order. Select rows and open them with `e` (one window) or `p` (per host); `j` opens the highlighted row at once.

Selections and typed text stay when you switch views; they are not kept across a restart.

## Keyboard

Single letters work while no text field has the focus.

| Key | Where | Action |
|---|---|---|
| `q` | everywhere | Quit |
| Esc | everywhere | Hide the window (in a filter: back to the list) |
| `n` | everywhere | New host |
| `o` | everywhere | Open view |
| `x` | everywhere | Open all detected sessions |
| `+` / `-` | everywhere | Direct session / Show all |
| Ctrl+F or `/` | everywhere | Filter of the current list |
| F5 | everywhere | Read the SSH config again, reconnect all |
| `l` | everywhere | Settings (language) |
| Ctrl+, | everywhere | Settings |
| F1 | everywhere | Explanations of all shortcuts |
| Del | hosts | Delete or hide the host |
| `t` / `a` / `i` | lists | Toggle / toggle all / invert |
| `j` | host, Show all, Direct session | Open highlighted session / Execute |
| `e` / `p` | Open, Show all, Direct session | One window / one window per host |
| `r` / `s` | host | Reload or reconnect / Set up host |
| Ctrl+Enter | host | Direct start |
| `h` / `s` | Show all | Sort by host / session |
| Ctrl+Shift+arrows | everywhere | Move splitters |

## Running in the background

Open-TmuxTabs keeps running after you close the window: closing and Esc only hide it. The **hotkey** (default **Ctrl+Alt+T**) shows the window or hides it again. The **tray icon** opens the window with a left click; a right click offers *Open*, *Settings…*, *About* and *Quit*. Starting the EXE a second time brings the running window to the front. With *Start with Windows* the app starts hidden at logon.

## Settings

Open them with `l`, Ctrl+, or the tray menu.

| Setting | Meaning | Default |
|---|---|---|
| Language | *Automatic* follows the Windows display language (English if it is not supported); switching takes effect at once | Automatic |
| Hotkey | Modifiers (Ctrl, Alt, Shift, Win) plus a key | Ctrl+Alt+T |
| Default user for known hosts | User for hosts from `known_hosts`; empty = your Windows user | empty |
| Background color | Base color; the light or dark theme follows from it | dark gray |
| Start with Windows | Adds the app to your autostart | off |

A hotkey already used by Windows or another program is rejected; the previous one stays active.

Settings, window size, splitter positions and your own hosts are stored in `OpenTmuxTabs.ini` next to the EXE (if that folder is read-only: `%APPDATA%\OpenTmuxTabs\OpenTmuxTabs.ini`). The text-mode Python variant uses its own files; both can be installed side by side.

## Uninstall

```powershell
irm https://raw.githubusercontent.com/schlingensiepen/tmux-helper-rs/main/deploy/uninstall.ps1 | iex
```

## FAQ

**A host stays red.** Read the error message on its page. Typical causes: the host is not reachable, the key is missing (use *Set up host*), the host fingerprint was never confirmed (run `ssh <host>` once in a terminal), or tmux is not installed.

**No tabs open.** Check that Windows Terminal is installed (`wt.exe`). If creating a session fails on a host, no tabs are opened for that host and an error is shown.

**The hotkey does not work.** It is probably used by another program; the app tells you at start. Choose another one in the settings.

**Klingon?** Yes, in Latin transcription, with loanwords for technical terms such as SSH, host and session.
