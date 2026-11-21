//! Account Switcher: adds a button to Horizon's multiplayer server list that opens a small screen
//! of saved offline nicknames, so a player can switch which one an offline-mode (cracked) server
//! sees them join as, and get back to the one they started with in one click.
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
    ClientHost, ClientTickPreEvent, Flow, Hooks, ScreenCharTypedPreEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent, ScreenMouseDraggedPreEvent,
    ScreenRenderPostEvent, Text,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use screen::AccountScreen;

/// Names this mod's own screen to `ClientHost::mod_screen_open`/`mod_screen`; must match the
/// `id` in `horizon-mod.toml`.
const MOD_ID: &str = "account_switcher";

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
    /// The mod's own screen, while it is open; rebuilt (via `screen::AccountScreen::refresh`)
    /// after every action so its rows and "aktív" markers always match disk.
    screen: Mutex<Option<AccountScreen>>,
    /// The trigger button on the multiplayer screen was pressed: the next client tick opens the
    /// mod screen (`mod_screen_open` needs `&mut dyn ClientHost`, which the read-only input hooks
    /// don't get -- same reason `close` below is applied on tick, as in the SDK's own
    /// `widget_screen` example).
    open: AtomicBool,
    /// Vissza was pressed: the next client tick closes the screen.
    close: AtomicBool,
}

impl AccountSwitcher {
    /// Runs `apply` on the open mod screen's widgets and acts on what it answers. Does nothing,
    /// and tells the caller to let the input through, when our screen isn't the one open.
    fn input(&self, host: &dyn ClientHost, apply: impl FnOnce(&mut Widgets) -> Outcome) -> Flow {
        if host.mod_screen() != Some(MOD_ID) {
            return Flow::Continue;
        }
        let outcome = {
            let mut guard = self.screen.lock().unwrap();
            let Some(screen) = guard.as_mut() else {
                return Flow::Continue;
            };
            apply(&mut screen.widgets)
        };
        self.act_on(host, outcome);
        if outcome.handled() {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    /// Applies a button press to the profile store and schedules a rebuild (`screen = None`, so
    /// the next `screen_render_post` calls `AccountScreen::refresh`) when anything changed.
    fn act_on(&self, host: &dyn ClientHost, outcome: Outcome) {
        let Outcome::Pressed(id) = outcome else {
            return;
        };
        let mut guard = self.screen.lock().unwrap();
        let Some(screen) = guard.as_ref() else {
            return;
        };

        if id == screen.back {
            self.close.store(true, Ordering::Relaxed);
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
    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        if self.open.swap(false, Ordering::Relaxed) {
            host.mod_screen_open(MOD_ID);
        }
        if self.close.swap(false, Ordering::Relaxed) {
            host.mod_screen_close(MOD_ID);
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if event.screen.id == MULTIPLAYER_SCREEN {
            let rect = trigger_rect(event.screen.rect[2]);
            let highlighted = contains(rect, event.screen.mouse);
            let _ = event
                .draw
                .widget_button(rect, &Text::literal("Fiókváltás"), true, highlighted);
            return;
        }

        if host.mod_screen() != Some(MOD_ID) {
            return;
        }
        let [width, height] = event.draw.size();
        let mut guard = self.screen.lock().unwrap();
        let screen =
            guard.get_or_insert_with(|| AccountScreen::refresh(host, String::new()));

        event.draw.fill([0.0, 0.0, width, height], 0xD000_0000);
        event
            .draw
            .text("Account Switcher", [20.0, 16.0], 0xFFFF_FFFF, true);
        event.draw.text(
            "Csak offline (cracked) szervereknél számít; bejelentkezett Microsoft-fiók vagy a --name kapcsoló felülírja.",
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
        if event.screen.id == MULTIPLAYER_SCREEN {
            if event.button == 0 && contains(trigger_rect(event.screen.rect[2]), event.screen.mouse) {
                *self.screen.lock().unwrap() = None;
                self.open.store(true, Ordering::Relaxed);
                return Flow::Cancel;
            }
            return Flow::Continue;
        }
        let (at, button, modifiers) = (event.screen.mouse, event.button, event.modifiers);
        self.input(host, |w| w.mouse_pressed(host, at, button, modifiers))
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        let button = event.button;
        self.input(host, |w| w.mouse_released(button))
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        let (at, button) = (event.screen.mouse, event.button);
        self.input(host, |w| w.mouse_dragged(host, at, button))
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        let (key, modifiers) = (event.key, event.modifiers);
        self.input(host, |w| w.key_pressed(host, key, modifiers))
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        let c = event.c;
        self.input(host, |w| w.char_typed(host, c))
    }
}
