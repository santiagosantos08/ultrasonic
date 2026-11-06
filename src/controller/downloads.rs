use super::state::{sync_ui_from_state, with_app_state, AppState};
use crate::*;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub fn register_download_callbacks(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Download Song
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_song(move |song_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                ui.set_status_message("Connect to Navidrome server to download tracks".into());
                return;
            }
            if let Some(song) = st.find_song_by_id(&song_id) {
                let dl_fmt = st.config.download_format.clone();
                let dl_br = st.config.download_max_bitrate;
                let client = st.client.clone();
                let ui_weak = ui.as_weak();
                ui.set_status_message(format!("Downloading '{}'...", song.title).into());

                std::thread::spawn(move || {
                    #[cfg(not(target_arch = "wasm32"))]
                    match client.fetch_download_bytes(&song.id, &dl_fmt, dl_br) {
                        Ok(bytes) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                let Some(ui) = ui_weak.upgrade() else { return };
                                with_app_state(move |st| {
                                    if st
                                        .offline
                                        .store_track(song.clone(), &bytes, &dl_fmt, dl_br)
                                        .is_ok()
                                    {
                                        ui.set_status_message(
                                            format!(
                                                "Downloaded '{}' ({} {})",
                                                song.title,
                                                dl_fmt.to_ascii_uppercase(),
                                                if dl_br == 0 {
                                                    "Original".to_string()
                                                } else {
                                                    format!("{}kbps", dl_br)
                                                }
                                            )
                                            .into(),
                                        );
                                        sync_ui_from_state(&ui, st);
                                    }
                                });
                            });
                        }
                        Err(err) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                let Some(ui) = ui_weak.upgrade() else { return };
                                ui.set_status_message(format!("Download failed: {}", err).into());
                            });
                        }
                    }
                });
            }
        });
    }

    // Download Album
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            let dl_fmt = st.config.download_format.clone();
            let dl_br = st.config.download_max_bitrate;
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            ui.set_status_message("Starting album download...".into());

            std::thread::spawn(move || {
                if let Ok(alb) = client.get_album(&aid) {
                    let mut count = 0;
                    for song in &alb.song {
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Ok(bytes) = client.fetch_download_bytes(&song.id, &dl_fmt, dl_br) {
                            let s_clone = song.clone();
                            let fmt_c = dl_fmt.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                with_app_state(|st| {
                                    let _ = st.offline.store_track(s_clone, &bytes, &fmt_c, dl_br);
                                });
                            });
                            count += 1;
                        }
                    }
                    let alb_title = alb.display_title();
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        with_app_state(move |st| {
                            ui.set_status_message(format!("Downloaded {} tracks from '{}'", count, alb_title).into());
                            sync_ui_from_state(&ui, st);
                        });
                    });
                }
            });
        });
    }

    // Download Selected Playlist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_selected_playlist(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let dl_fmt = st.config.download_format.clone();
            let dl_br = st.config.download_max_bitrate;
            let songs = st.current_playlist_songs.clone();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            ui.set_status_message(format!("Downloading {} playlist tracks...", songs.len()).into());

            std::thread::spawn(move || {
                let mut count = 0;
                for s in songs {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Ok(bytes) = client.fetch_download_bytes(&s.id, &dl_fmt, dl_br) {
                        let s_clone = s.clone();
                        let fmt_c = dl_fmt.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_state(|st| {
                                let _ = st.offline.store_track(s_clone, &bytes, &fmt_c, dl_br);
                            });
                        });
                        count += 1;
                    }
                }
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        ui.set_status_message(format!("Downloaded {} playlist tracks", count).into());
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Clear Downloads
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_clear_downloads(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.offline.clear_all();
            ui.set_status_message("Cleared local offline downloads".into());
            sync_ui_from_state(&ui, &mut st);
        });
    }
}
