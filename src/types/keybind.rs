use serde::{Deserialize, Serialize};

/// A user-configurable key binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl KeyBinding {
    pub fn key(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: false,
            shift: false,
            alt: false,
        }
    }
    pub fn ctrl(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: true,
            shift: false,
            alt: false,
        }
    }
    pub fn shift(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: false,
            shift: true,
            alt: false,
        }
    }
    pub fn alt(key: &str) -> Self {
        KeyBinding {
            key: key.into(),
            ctrl: false,
            shift: false,
            alt: true,
        }
    }

    /// Check if a KeyEvent matches this binding.
    pub fn matches_event(&self, key: &crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mods_match = key.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && key.modifiers.contains(KeyModifiers::SHIFT) == self.shift
            && key.modifiers.contains(KeyModifiers::ALT) == self.alt;
        if !mods_match {
            return false;
        }
        match &key.code {
            KeyCode::Char(c) => self.key == c.to_string(),
            KeyCode::Enter => self.key == "enter",
            KeyCode::Esc => self.key == "esc",
            KeyCode::Up => self.key == "up",
            KeyCode::Down => self.key == "down",
            KeyCode::Backspace => self.key == "backspace",
            KeyCode::Tab => self.key == "tab",
            KeyCode::F(n) => self.key == format!("f{n}"),
            _ => false,
        }
    }
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(&self.key);
        s
    }
}

/// All configurable key bindings with defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keybinds {
    #[serde(default = "Keybinds::default_move_up")]
    pub move_up: KeyBinding,
    #[serde(default = "Keybinds::default_move_down")]
    pub move_down: KeyBinding,
    #[serde(default = "Keybinds::default_expand")]
    pub expand: KeyBinding,
    #[serde(default = "Keybinds::default_refresh")]
    pub refresh: KeyBinding,
    #[serde(default = "Keybinds::default_rename")]
    pub rename: KeyBinding,
    #[serde(default = "Keybinds::default_new_workspace")]
    pub new_workspace: KeyBinding,
    #[serde(default = "Keybinds::default_delete")]
    pub delete: KeyBinding,
    #[serde(default = "Keybinds::default_new_session")]
    pub new_session: KeyBinding,
    #[serde(default = "Keybinds::default_search")]
    pub search: KeyBinding,
    #[serde(default = "Keybinds::default_help")]
    pub help: KeyBinding,
    #[serde(default = "Keybinds::default_settings")]
    pub settings: KeyBinding,
    #[serde(default = "Keybinds::default_theme")]
    pub theme: KeyBinding,
    #[serde(default = "Keybinds::default_export")]
    pub export: KeyBinding,
    #[serde(default = "Keybinds::default_copy")]
    pub copy: KeyBinding,
    #[serde(default = "Keybinds::default_preview")]
    pub preview: KeyBinding,
    #[serde(default = "Keybinds::default_tag_filter")]
    pub tag_filter: KeyBinding,
    #[serde(default = "Keybinds::default_quit")]
    pub quit: KeyBinding,
}

impl Default for Keybinds {
    fn default() -> Self {
        Keybinds {
            move_up: Keybinds::default_move_up(),
            move_down: Keybinds::default_move_down(),
            expand: Keybinds::default_expand(),
            refresh: Keybinds::default_refresh(),
            rename: Keybinds::default_rename(),
            new_workspace: Keybinds::default_new_workspace(),
            delete: Keybinds::default_delete(),
            new_session: Keybinds::default_new_session(),
            search: Keybinds::default_search(),
            help: Keybinds::default_help(),
            settings: Keybinds::default_settings(),
            theme: Keybinds::default_theme(),
            export: Keybinds::default_export(),
            copy: Keybinds::default_copy(),
            preview: Keybinds::default_preview(),
            tag_filter: Keybinds::default_tag_filter(),
            quit: Keybinds::default_quit(),
        }
    }
}

impl Keybinds {
    fn default_move_up() -> KeyBinding {
        KeyBinding::key("up")
    }
    fn default_move_down() -> KeyBinding {
        KeyBinding::key("down")
    }
    fn default_expand() -> KeyBinding {
        KeyBinding::alt("e")
    }
    fn default_refresh() -> KeyBinding {
        KeyBinding::alt("r")
    }
    fn default_rename() -> KeyBinding {
        KeyBinding::alt("m")
    }
    fn default_new_workspace() -> KeyBinding {
        KeyBinding::alt("w")
    }
    fn default_delete() -> KeyBinding {
        KeyBinding::alt("d")
    }
    fn default_new_session() -> KeyBinding {
        KeyBinding::alt("n")
    }
    fn default_search() -> KeyBinding {
        KeyBinding::alt("/")
    }
    fn default_help() -> KeyBinding {
        KeyBinding::alt("k")
    }
    fn default_settings() -> KeyBinding {
        KeyBinding::alt("s")
    }
    fn default_theme() -> KeyBinding {
        KeyBinding::alt("t")
    }
    fn default_export() -> KeyBinding {
        KeyBinding::alt("x")
    }
    fn default_copy() -> KeyBinding {
        KeyBinding::alt("y")
    }
    fn default_preview() -> KeyBinding {
        KeyBinding::alt("v")
    }
    fn default_tag_filter() -> KeyBinding {
        KeyBinding::alt("f")
    }
    fn default_quit() -> KeyBinding {
        KeyBinding::alt("q")
    }
    /// Detect keybind conflicts. Returns a list of (action_a, action_b) pairs
    /// that share the same key binding.
    pub fn validate(&self) -> Vec<(&'static str, &'static str)> {
        let bindings: Vec<(&str, &KeyBinding)> = vec![
            ("move_up", &self.move_up),
            ("move_down", &self.move_down),
            ("expand", &self.expand),
            ("refresh", &self.refresh),
            ("rename", &self.rename),
            ("new_workspace", &self.new_workspace),
            ("delete", &self.delete),
            ("new_session", &self.new_session),
            ("search", &self.search),
            ("help", &self.help),
            ("settings", &self.settings),
            ("theme", &self.theme),
            ("export", &self.export),
            ("copy", &self.copy),
            ("preview", &self.preview),
            ("tag_filter", &self.tag_filter),
            ("quit", &self.quit),
        ];
        let mut conflicts = Vec::new();
        for i in 0..bindings.len() {
            for j in (i + 1)..bindings.len() {
                let (name_a, kb_a) = bindings[i];
                let (name_b, kb_b) = bindings[j];
                if kb_a.key == kb_b.key
                    && kb_a.ctrl == kb_b.ctrl
                    && kb_a.shift == kb_b.shift
                    && kb_a.alt == kb_b.alt
                {
                    conflicts.push((name_a, name_b));
                }
            }
        }
        conflicts
    }
    /// Return a formatted list of all keybindings for display.
    pub fn display_lines(&self) -> Vec<String> {
        vec![
            format!("  move_up:       {}", self.move_up.display()),
            format!("  move_down:     {}", self.move_down.display()),
            format!("  expand:        {}", self.expand.display()),
            format!("  refresh:       {}", self.refresh.display()),
            format!("  rename:        {}", self.rename.display()),
            format!("  new_workspace: {}", self.new_workspace.display()),
            format!("  delete:        {}", self.delete.display()),
            format!("  new_session:   {}", self.new_session.display()),
            format!("  search:        {}", self.search.display()),
            format!("  keybinds:      {}", self.help.display()),
            format!("  settings:      {}", self.settings.display()),
            format!("  theme:         {}", self.theme.display()),
            format!("  export:        {}", self.export.display()),
            format!("  copy:          {}", self.copy.display()),
            format!("  preview:       {}", self.preview.display()),
            format!("  tag_filter:    {}", self.tag_filter.display()),
            format!("  quit:          {}", self.quit.display()),
        ]
    }
    /// One-line hint string for the status bar.
    pub fn status_hint(&self) -> String {
        format!(
            "Enter:open Tab:focus {}:search {}:help {}:quit",
            self.search.display(),
            self.help.display(),
            self.quit.display(),
        )
    }
    /// Key/action pairs for the help popup sidebar section (plain strings).
    pub fn help_sidebar_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "Move selection",
                format!("{}/{} ↑↓", self.move_up.display(), self.move_down.display()),
            ),
            ("New session / Resume / Switch", "Enter".into()),
            ("Expand / collapse", self.expand.display()),
            ("Refresh sessions", self.refresh.display()),
            ("Rename selected", self.rename.display()),
            ("New workspace", self.new_workspace.display()),
            ("Delete", self.delete.display()),
            ("New session (agent picker)", self.new_session.display()),
            ("Search sessions", self.search.display()),
            ("This help", self.help.display()),
            ("Quit", format!("{} / Esc", self.quit.display())),
        ]
    }
}
