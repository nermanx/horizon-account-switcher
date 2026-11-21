//! Account Switcher: adds a button to Horizon's multiplayer server list that opens a small screen
//! of saved offline nicknames, so a player can switch which one an offline-mode (cracked) server
//! sees them join as, and get back to the one they started with in one click.
//!
//! ## Why this draws its own overlay instead of `ClientHost::mod_screen_open`
//! The mod-screen API (`mod_screen_open`/`mod_screen_close`/`mod_screen`) only works "over the
//! world": its own doc says it returns `false` (opens nothing) "over the title screen" -- and the
//! server list (`multiplayer`) is part of the title's screen state machine, not a world. Using it
//! here would silently no-op, which is exactly the bug this file now avoids: this mod keeps its
//! own `Option<AccountScreen>` as the single source of truth for "is my UI open", draws it with
//! `screen_render_post` directly on top of the still-open vanilla `multiplayer` screen, and
//! cancels every mouse/key/char event reaching that screen while it is open, the same way a
//! client-side overlay mod (a tooltip, a HUD panel) already works without ever calling
//! `mod_screen_open`.
//!
//! ## What this does and does not do
//! Horizon (like vanilla and every other Minecraft client) only ever enforces a *verified*
//! identity for a signed-in Microsoft account: on such a connection the account's own name always
//! wins, whatever this mod does (`horizon_client`'s connection code forces it, see the README).
//! An **offline**-mode nickname, by contrast, was never authenticated to begin with -- it is a
//! plain string the client already lets a player edit by hand in `horizon-player.txt` next to
//! `options.txt` (the engine's own `offline_name.rs` says so explicitly). This mod only adds a UI
//! for picking among several such strings instead of editing that file by hand; it cannot and does
//! not touch, forge or bypass any authenticated account or any server-side ban/anticheat system.
//!
//! See `player_name.rs`, `profiles.rs` and `screen.rs` for the pieces, and the README for the
//! exact engine behaviour this relies on, with file and line citations.

mod player_name;
mod profiles;
mod screen;
mod sha256;

use mod_api::client::widgets::{Outcome, Widgets};
use mod_api::{
    ClientHost, Flow, Hooks, ScreenCharTypedPreEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent, ScreenMouseDraggedPreEvent,
    ScreenRenderPostEvent, Text,
};
use std::sync::Mutex;

use screen::AccountScreen;

/// `ScreenView::id` of Horizon's server-list screen (`JoinMultiplayerScreen`), stable across
/// releases by the screen hooks' own contract. See `crates/horizon-client/src/multiplayer.rs`
/// `Screen::id` in the Horizon source (cited in the README).
const MULTIPLAYER_SCREEN: &str = "multiplayer";

const TRIGGER_WIDTH: f32 = 112.0;
const TRIGGER_HEIGHT: f32 = 20.0;
const TRIGGER_MARGIN: f32 = 6.0;

/// The trigger button's rect, top-right corner of the server-list screen: free space on every
/// window size the game allows (the server list and its two footer button rows stay well clear).
fn trigger_rect(screen_width: f32) -> [f32; 4] {
    [
        screen_width - TRIGGER_WIDTH - TRIGGER_MARGIN,
        TRIGGER_MARGIN,
        TRIGGER_WIDTH,
        TRIGGER_HEIGHT,
    ]
}

/// `AbstractWidget.isMouseOver`-style hit test (mirrors `mod_api::client::widgets`'s private
/// `over`, which this crate cannot reach from outside that module).
fn contains([x, y, w, h]: [f32; 4], [px, py]: [f32; 2]) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}

#[derive(Default)]
pub struct AccountSwitcher {
    /// `Some` while our overlay is open on top of the `multiplayer` screen; `None` shows just the
    /// trigger button. This is the mod's own state, not `ClientHost::mod_screen()` (see the
    /// module docs for why).
    screen: Mutex<Option<AccountScreen>>,
}

impl AccountSwitcher {
    /// Runs `apply` on the open overlay's widgets and acts on what it answers, when the overlay
    /// is open. Returns whether it was open (so the caller knows whether it owned the input).
    fn input(&self, host: &dyn ClientHost, apply: impl FnOnce(&mut Widgets) -> Outcome) -> bool {
        let outcome = {
            let mut guard = self.screen.lock().unwrap();
            let Some(screen) = guard.as_mut() else {
                return false;
            };
            apply(&mut screen.widgets)
        };
        self.act_on(host, outcome);
        true
    }

    /// Applies a button press to the profile store and rebuilds the overlay
    /// (`AccountScreen::refresh`) so its rows and the "aktív" marker always match disk right
    /// after the change.
    fn act_on(&self, host: &dyn ClientHost, outcome: Outcome) {
        let Outcome::Pressed(id) = outcome else {
            return;
        };
        let mut guard = self.screen.lock().unwrap();
        let Some(screen) = guard.as_ref() else {
            return;
        };

        if id == screen.back {
            *guard = None;
            return;
        }

        if id == screen.save_as_new {
            let typed = screen
                .widgets
                .text_field(screen.new_name)
                .map(|f| f.value().trim().to_owned())
                .unwrap_or_default();
            let message = if typed.is_empty() {
                "Adj meg egy nevet a mentéshez.".to_owned()
            } else if !player_name::is_valid_name(&typed) {
                "Érvénytelen név: 1-16 karakter, csak betű, szám és \"_\".".to_owned()
            } else {
                let mut profiles = profiles::load(host);
                if profiles.iter().any(|p| p.name == typed) {
                    format!("\"{typed}\" már mentve van; lent váltható aktívra.")
                } else {
                    let main = profiles.is_empty();
                    profiles.push(profiles::Profile {
                        name: typed.clone(),
                        label: typed.clone(),
                        main,
                        created_at_ms: host.millis(),
                    });
                    let saved = profiles::save(host, &profiles);
                    let _ = player_name::write_active(&typed);
                    if saved {
                        format!("Mentve és aktiválva: {typed}")
                    } else {
                        format!("Aktiválva: {typed} (a mentés nem sikerült, csak ebben a munkamenetben él)")
                    }
                }
            };
            *guard = Some(AccountScreen::refresh(host, message));
            return;
        }

        if let Some(row) = screen.rows.iter().find(|r| r.switch == id) {
            let name = row.name.clone();
            let message = if player_name::write_active(&name).is_ok() {
                format!("Átváltva erre: {name}")
            } else {
                format!("Nem sikerült írni a horizon-player.txt fájlt ehhez: {name}")
            };
            *guard = Some(AccountScreen::refresh(host, message));
            return;
        }

        if let Some(row_index) = screen.rows.iter().position(|r| r.delete == Some(id)) {
            let name = screen.rows[row_index].name.clone();
            let mut profiles = profiles::load(host);
            profiles.retain(|p| p.name != name || p.main);
            profiles::save(host, &profiles);
            let message = format!("Törölve: {name}");
            *guard = Some(AccountScreen::refresh(host, message));
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for AccountSwitcher {
    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if event.screen.id != MULTIPLAYER_SCREEN {
            return;
        }
        let [width, height] = event.draw.size();
        let mut guard = self.screen.lock().unwrap();

        let Some(screen) = guard.as_mut() else {
            // Not open: just the trigger button, top-right.
            let rect = trigger_rect(width);
            let highlighted = contains(rect, event.screen.mouse);
            let _ = event
                .draw
                .widget_button(rect, &Text::literal("Fiókváltás"), true, highlighted);
            return;
        };

        event.draw.fill([0.0, 0.0, width, height], 0xD000_0000);
        event
            .draw
            .text("Account Switcher", [20.0, 16.0], 0xFFFF_FFFF, true);
        event.draw.text(
            "Csak offline (cracked) szervereknél számít; bejelentkezett Microsoft-fiók vagy a --name kapcsoló felülírja. (Esc vagy Vissza bezárja.)",
            [20.0, height - 40.0],
            0xFF90_9090,
            true,
        );
        if !screen.message.is_empty() {
            event
                .draw
                .text(&screen.message, [20.0, height - 56.0], 0xFF80_FF80, true);
        }
        let _ = screen.widgets.draw(&mut *event.draw, host, event.screen.mouse);
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if event.screen.id != MULTIPLAYER_SCREEN {
            return Flow::Continue;
        }
        let (at, button, modifiers) = (event.screen.mouse, event.button, event.modifiers);
        if self.input(host, |w| w.mouse_pressed(host, at, button, modifiers)) {
            // The overlay is open: it owns every click on this screen while it is up, so the
            // server list underneath never also reacts to the same press.
            return Flow::Cancel;
        }
        if button == 0 && contains(trigger_rect(event.screen.rect[2]), event.screen.mouse) {
            *self.screen.lock().unwrap() = Some(AccountScreen::refresh(host, String::new()));
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        if event.screen.id != MULTIPLAYER_SCREEN {
            return Flow::Continue;
        }
        let button = event.button;
        if self.input(host, |w| w.mouse_released(button)) {
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        if event.screen.id != MULTIPLAYER_SCREEN {
            return Flow::Continue;
        }
        let (at, button) = (event.screen.mouse, event.button);
        if self.input(host, |w| w.mouse_dragged(host, at, button)) {
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if event.screen.id != MULTIPLAYER_SCREEN {
            return Flow::Continue;
        }
        if event.key == "key.keyboard.escape" && self.screen.lock().unwrap().is_some() {
            *self.screen.lock().unwrap() = None;
            return Flow::Cancel;
        }
        let (key, modifiers) = (event.key, event.modifiers);
        if self.input(host, |w| w.key_pressed(host, key, modifiers)) {
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        if event.screen.id != MULTIPLAYER_SCREEN {
            return Flow::Continue;
        }
        let c = event.c;
        if self.input(host, |w| w.char_typed(host, c)) {
            return Flow::Cancel;
        }
        Flow::Continue
    }
}
