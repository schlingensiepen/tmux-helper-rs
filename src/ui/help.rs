//! Footer entries (shortcut bar) and the explanation overlay (SPEC-003 §6.3).

use super::{Action, View};
use crate::core::i18n::tr;

/// One shortcut shown in the footer: key label, name and explanation
/// (msgids), and the action a click triggers.
#[derive(Clone, Copy)]
pub struct Item {
    pub key: &'static str,
    pub name: &'static str,
    pub help: &'static str,
    pub action: Action,
}

const fn it(key: &'static str, name: &'static str, help: &'static str, action: Action) -> Item {
    Item {
        key,
        name,
        help,
        action,
    }
}

const GLOBAL: &[Item] = &[
    it("q", "Quit", "Quits Open-TmuxTabs. The sessions on the hosts keep running. Closing the window or pressing Esc only hides it in the notification area.", Action::Quit),
    it("n", "New host", "Jumps to \"New host\" and puts the cursor in the input field. Enter an SSH target such as user@server or an alias from your SSH config.", Action::NewHost),
    it("o", "Open", "Jumps to the Open view: summary of your selection and the quick start button.", Action::GotoOpen),
    it("x", "Open all", "Opens all running sessions on all reachable hosts right away, one Windows Terminal window per host. Waits for hosts that are still connecting.", Action::OpenAll),
    it("+", "Direct session", "Jumps to Direct session: type session names once and start them on several hosts at the same time.", Action::GotoDirect),
    it("-", "Show all", "Jumps to Show all: one sortable table with the sessions of all reachable hosts.", Action::GotoShowAll),
    it("Ctrl+F", "Filter", "Moves the cursor into the search field above the current list. The list shows only entries that contain the text (upper and lower case do not matter). Esc or Arrow down returns to the list.", Action::Filter),
    it("F5", "Reload hosts", "Reads your SSH config file again and reconnects to all hosts.", Action::ReloadConfig),
    it("l", "Settings", "Opens the settings: language, hotkey, background color and start with Windows.", Action::SettingsLanguage),
    it("F1", "Help", "Shows the explanations of all shortcuts of the current view.", Action::Help),
];

const DEL: Item = it("Del", "Delete host", "Removes the selected host after a confirmation. A host from your SSH config is only hidden in the app; the config file is not changed.", Action::DeleteHost);
const TOGGLE: Item = it(
    "t",
    "Toggle",
    "Selects or deselects the highlighted entry.",
    Action::Toggle,
);
const TOGGLE_ALL: Item = it("a", "Toggle All", "Selects all visible entries, or deselects them if all are selected already. Entries hidden by the filter stay as they are.", Action::ToggleAll);
const INVERT: Item = it(
    "i",
    "Invert",
    "Inverts the selection of the visible entries.",
    Action::Invert,
);

pub fn items(view: View) -> Vec<Item> {
    let mut v: Vec<Item> = Vec::new();
    match view {
        View::NewHost => {
            v.push(it(
                "Enter",
                "Add host",
                "Adds the typed SSH target to the list and connects to it in the background.",
                Action::AddHost,
            ));
            v.push(it(
                "Tab",
                "Known hosts",
                "The list below offers hosts from your known_hosts file that are not in the list yet, and all hidden hosts. Enter or double-click copies one into the input field; change the user if needed and press Enter to add it.",
                Action::Filter,
            ));
        }
        View::Pending => v.push(DEL),
        View::Failed => {
            v.push(it(
                "r",
                "Reconnect",
                "Tries to connect to the host again.",
                Action::Reload,
            ));
            v.push(it("s", "Set up host", "Opens an interactive Windows Terminal window that copies your public SSH key to the host. You type the password once; afterwards the host connects without a password.", Action::SetupHost));
            v.push(DEL);
        }
        View::Connected => {
            v.extend([TOGGLE, TOGGLE_ALL, INVERT]);
            v.push(it(
                "j",
                "Jump start",
                "Opens the highlighted session immediately in a Windows Terminal tab. The selection stays as it is.",
                Action::JumpStart,
            ));
            v.push(it(
                "r",
                "Reload",
                "Fetches the session list from the host again.",
                Action::Reload,
            ));
            v.push(it("Ctrl+Enter", "Direct start", "Creates the sessions typed in the field below on this host and opens them at once in one Windows Terminal window. Existing sessions with the same name are attached.", Action::DirectStart));
            v.push(DEL);
        }
        View::Open => {
            v.push(it("e", "One WT", "Opens all selected sessions, and creates the new ones, in a single Windows Terminal window. Afterwards the selection is cleared.", Action::OneWindow));
            v.push(it(
                "p",
                "Per host",
                "Like One WT, but with one Windows Terminal window per host.",
                Action::PerHost,
            ));
        }
        View::Direct => {
            v.extend([TOGGLE, TOGGLE_ALL, INVERT]);
            v.push(it("j", "Execute", "Creates the missing sessions on the selected hosts and opens all of them in one Windows Terminal window. Selection and names stay, so you can run it again.", Action::Execute));
            v.push(it(
                "p",
                "Per host",
                "Like Execute, but with one Windows Terminal window per host.",
                Action::PerHost,
            ));
        }
        View::ShowAll => {
            v.extend([TOGGLE, TOGGLE_ALL, INVERT]);
            v.push(it(
                "j",
                "Jump start",
                "Opens the highlighted session immediately in a Windows Terminal tab. The selection stays as it is.",
                Action::JumpStart,
            ));
            v.push(it(
                "e",
                "One WT",
                "Opens all selected sessions in a single Windows Terminal window. The selection stays.",
                Action::OneWindow,
            ));
            v.push(it(
                "p",
                "Per host",
                "Opens the selected sessions with one Windows Terminal window per host. The selection stays.",
                Action::PerHost,
            ));
            v.push(it(
                "h",
                "Sort by host",
                "Sorts the table by host. Press again to reverse the order.",
                Action::SortHost,
            ));
            v.push(it(
                "s",
                "Sort by session",
                "Sorts the table by session name. Press again to reverse the order.",
                Action::SortSession,
            ));
        }
    }
    v.extend_from_slice(GLOBAL);
    v
}

/// Key label as shown (Ctrl/Enter/Del are translated).
pub fn key_label(key: &str) -> String {
    key.replace("Ctrl", &tr("Ctrl"))
        .replace("Enter", &tr("Enter"))
        .replace("Del", &tr("Del"))
}

/// Text of the overlay for one entry.
pub fn text_one(i: &Item) -> String {
    format!("{}  {}\n\n{}", key_label(i.key), tr(i.name), tr(i.help))
}

/// Text of the overlay with all entries of a view.
pub fn text_all(view: View) -> String {
    items(view)
        .iter()
        .map(|i| format!("{}  {}\n    {}", key_label(i.key), tr(i.name), tr(i.help)))
        .collect::<Vec<_>>()
        .join("\n\n")
}
