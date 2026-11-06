use super::playback::play_track_by_id;
use super::state::{sync_ui_from_state, with_app_state, AppState};
use crate::models::format_duration;
use crate::*;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub fn register_playlist_callbacks(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Select Playlist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_playlist(move |pl_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = pl_id.to_string();
            ui.set_is_loading(true);
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let pl = client.get_playlist(&pid).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pl) = pl {
                            ui.set_selected_playlist_id(pl.id.into());
                            ui.set_selected_playlist_name(pl.name.into());
                            ui.set_selected_playlist_count(pl.song_count as i32);
                            ui.set_selected_playlist_duration(format_duration(pl.duration).into());
                            st.current_playlist_songs = pl.entry;
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Create New Playlist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_create_new_playlist(move |name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            ui.set_new_playlist_name("".into());
            ui.set_status_message(format!("Creating playlist '{}'...", trimmed).into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.create_playlist(&trimmed, &[]);
                let pls = client.get_playlists().ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pls) = pls {
                            st.current_playlists = pls;
                        }
                        ui.set_status_message(format!("Created playlist '{}'", trimmed).into());
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Add Current Song to Playlist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_add_current_song_to_playlist(move |pl_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = pl_id.to_string();
            let Some(curr_id) = st.audio.current_song_id.clone() else { return };
            let Some(song) = st.find_song_by_id(&curr_id) else { return };
            let song_title = song.title.clone();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.update_playlist(&pid, &[curr_id], &[]);
                let pl = client.get_playlist(&pid).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pl) = pl {
                            st.current_playlist_songs = pl.entry;
                        }
                        ui.set_status_message(format!("Added '{}' to playlist", song_title).into());
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Remove Song from Playlist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_remove_song_from_selected_playlist(move |row_idx| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = ui.get_selected_playlist_id().to_string();
            let idx = row_idx as usize;
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.update_playlist(&pid, &[], &[idx]);
                let pl = client.get_playlist(&pid).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pl) = pl {
                            st.current_playlist_songs = pl.entry;
                        }
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Delete Playlist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_selected_playlist(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = ui.get_selected_playlist_id().to_string();
            if pid.is_empty() {
                return;
            }
            ui.set_selected_playlist_id("".into());
            ui.set_selected_playlist_name("".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.delete_playlist(&pid);
                let pls = client.get_playlists().ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pls) = pls {
                            st.current_playlists = pls;
                        }
                        st.current_playlist_songs.clear();
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Save Queue to Server
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_save_queue_to_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let ids: Vec<String> = st.queue.iter().map(|s| s.id.clone()).collect();
            let curr = st.audio.current_song_id.clone();
            let pos_ms = (st.audio.elapsed_secs() * 1000.0) as u64;
            let client = st.client.clone();
            std::thread::spawn(move || {
                let _ = client.save_play_queue(&ids, curr.as_deref(), pos_ms);
            });
            ui.set_status_message("Play queue synced to Navidrome server".into());
        });
    }

    // Load Queue from Server
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_load_queue_from_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            ui.set_status_message("Loading play queue from server...".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let pq = client.get_play_queue().ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pq) = pq {
                            st.queue = pq.entry;
                            if let Some(curr_id) = pq.current {
                                play_track_by_id(&ui, st, &curr_id);
                            }
                            ui.set_status_message("Loaded play queue from server".into());
                            sync_ui_from_state(&ui, st);
                        }
                    });
                });
            });
        });
    }

    // Clear Queue
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_clear_queue(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.queue.clear();
            sync_ui_from_state(&ui, &mut st);
        });
    }
}
