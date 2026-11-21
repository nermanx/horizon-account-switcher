//! The mod's own screen (`ClientHost::mod_screen_open`): the saved nickname list, a field to add
//! a new one, and the buttons that drive them. Laid out and drawn exactly as
//! `docs/modding/examples/src/widget_screen.rs` in the Horizon source shows (the SDK's own
//! reference pattern for a mod screen with buttons and a text field).

use mod_api::client::widgets::{Button, TextField, WidgetId, Widgets};
use mod_api::{ClientHost, Text};

use crate::player_name;
use crate::profiles::{self, Profile};

const LEFT: f32 = 20.0;
const FIELD_Y: f32 = 46.0;
const FIRST_ROW_Y: f32 = 80.0;
const ROW_HEIGHT: f32 = 24.0;
const ROW_WIDTH: f32 = 260.0;
const DELETE_WIDTH: f32 = 70.0;
const DELETE_GAP: f32 = 6.0;

/// One saved profile's row: its name (to act on; labels can repeat) and its widgets.
pub struct Row {
    pub name: String,
    pub switch: WidgetId,
    pub delete: Option<WidgetId>,
}

pub struct AccountScreen {
    pub widgets: Widgets,
    pub new_name: WidgetId,
    pub save_as_new: WidgetId,
    pub back: WidgetId,
    pub rows: Vec<Row>,
    /// A status line from the last action (mentve / átváltva / törölve, or a refusal).
    pub message: String,
}

impl AccountScreen {
    /// Builds the screen's widgets from the profiles saved so far. Called again, with a fresh
    /// `message`, after every action that changes the list or the active name, so labels (the
    /// "aktív" marker) and rows always match what is actually on disk.
    fn build(profiles: &[Profile], active_name: Option<&str>, message: String) -> Self {
        let mut widgets = Widgets::new();
        let new_name = widgets.add_text_field(TextField::new([LEFT, FIELD_Y, 200.0, 20.0], 16));
        let save_as_new = widgets.add_button(Button::new(
            [LEFT + 206.0, FIELD_Y, 170.0, 20.0],
            Text::literal("Mentés új névként"),
        ));

        let mut rows = Vec::with_capacity(profiles.len());
        let mut y = FIRST_ROW_Y;
        for profile in profiles {
            let is_active = active_name == Some(profile.name.as_str());
            let mut label = profile.label.clone();
            if profile.main {
                label.push_str(" (eredeti / fiók)");
            }
            if is_active {
                label.push_str(" — aktív");
            }
            let switch = widgets.add_button(Button::new([LEFT, y, ROW_WIDTH, 20.0], Text::literal(label)));
            let delete = (!profile.main).then(|| {
                widgets.add_button(Button::new(
                    [LEFT + ROW_WIDTH + DELETE_GAP, y, DELETE_WIDTH, 20.0],
                    Text::literal("Törlés"),
                ))
            });
            rows.push(Row {
                name: profile.name.clone(),
                switch,
                delete,
            });
            y += ROW_HEIGHT;
        }

        let back = widgets.add_button(Button::new([LEFT, y + 10.0, 100.0, 20.0], Text::literal("Vissza")));

        Self {
            widgets,
            new_name,
            save_as_new,
            back,
            rows,
            message,
        }
    }

    /// Rebuilds from the profile store on disk (via `host`) and the currently active nickname,
    /// keeping `message`. Used the first time the screen opens and after every action.
    pub fn refresh(host: &dyn ClientHost, mut message: String) -> Self {
        let profiles = profiles::ensure_main(host);
        let active = player_name::read_active();
        if active.is_none() {
            if !message.is_empty() {
                message.push('\n');
            }
            message.push_str("Még nincs beállítva offline név ehhez a játékoshoz.");
        }
        Self::build(&profiles, active.as_deref(), message)
    }
}
