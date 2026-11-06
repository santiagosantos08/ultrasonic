pub mod catalog;
pub mod detail;
pub mod downloads;
pub mod playback;
pub mod playlists;
pub mod settings;
pub mod state;
pub mod timer;

pub use catalog::refresh_catalog_for_active_libraries;
pub use playback::{advance_track, play_track_by_id};
pub use state::{sync_ui_from_state, with_app_state, AppState, CURRENT_STATE};

use crate::*;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub fn attach_callbacks_and_run(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Register global current state for thread-safe UI callbacks
    CURRENT_STATE.with(|cell| {
        *cell.borrow_mut() = Some(state.clone());
    });

    // Attempt connection on startup if credentials exist in config.json; otherwise open Settings view
    {
        let st = state.borrow();
        if st.client.is_configured() {
            ui.set_is_loading(true);
            ui.set_active_view("listen_now".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                match client.ping() {
                    Ok(ping_res) => {
                        let folders = client.get_music_folders().ok().unwrap_or_default();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.is_connected = true;
                                ui.set_is_connected(true);
                                let srv_ver = ping_res
                                    .server_version
                                    .unwrap_or_else(|| ping_res.version.clone());
                                ui.set_server_version_badge(format!("Navidrome {}", srv_ver).into());
                                st.libraries = folders;
                                if st.config.selected_library_ids.is_empty() {
                                    st.config.selected_library_ids =
                                        st.libraries.iter().map(|l| l.id.clone()).collect();
                                }
                                ui.set_status_message("Connected to Navidrome server".into());
                                refresh_catalog_for_active_libraries(&ui, st);
                            });
                        });
                    }
                    Err(err) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.is_connected = false;
                                ui.set_is_connected(false);
                                ui.set_is_loading(false);
                                ui.set_active_view("settings".into());
                                ui.set_status_message(format!("Could not connect to Navidrome: {}", err).into());
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    }
                }
            });
        } else {
            let mut st = state.borrow_mut();
            st.is_connected = false;
            ui.set_is_connected(false);
            ui.set_active_view("settings".into());
            ui.set_status_message("".into());
            sync_ui_from_state(ui, &mut st);
        }
    }

    // Register all modular callbacks
    catalog::register_catalog_callbacks(ui, state.clone());
    playback::register_playback_callbacks(ui, state.clone());
    detail::register_detail_callbacks(ui, state.clone());
    playlists::register_playlist_callbacks(ui, state.clone());
    downloads::register_download_callbacks(ui, state.clone());
    settings::register_settings_callbacks(ui, state.clone());
    timer::start_playback_timer(ui, state);
}
