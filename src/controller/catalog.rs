use super::state::{sync_ui_from_state, with_app_state, AppState};
use crate::*;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub fn refresh_catalog_for_active_libraries(ui: &AppWindow, state: &mut AppState) {
    if !state.is_connected {
        sync_ui_from_state(ui, state);
        return;
    }

    ui.set_is_loading(true);
    let client = state.client.clone();
    let lib_ids = state.active_library_ids();
    let ui_weak = ui.as_weak();

    std::thread::spawn(move || {
        let albs = client.get_album_list2("newest", 40, 0, &lib_ids).ok();
        let rand_songs = client.get_random_songs(80, &lib_ids).ok();
        let arts = client.get_artists(&lib_ids).ok();
        let genres = client.get_genres().ok();
        let pls = client.get_playlists().ok();
        let shares = client.get_shares().ok();
        let scan = client.get_scan_status().ok();

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            with_app_state(move |st| {
                if let Some(albs) = albs {
                    st.featured_albums = albs.clone();
                    st.current_albums = albs;
                }
                if let Some(songs) = rand_songs {
                    st.current_songs = songs;
                }
                if let Some(arts) = arts {
                    st.current_artists = arts;
                }
                if let Some(genres) = genres {
                    st.current_genres = genres;
                }
                if let Some(pls) = pls {
                    st.current_playlists = pls;
                }
                if let Some(shares) = shares {
                    st.current_shares = shares;
                }
                if let Some(scan) = scan {
                    ui.set_scan_in_progress(scan.scanning);
                    ui.set_scan_count(scan.count.unwrap_or(0) as i32);
                    ui.set_scan_last_time(scan.last_scan.unwrap_or_else(|| "Idle".to_string()).into());
                }
                ui.set_is_loading(false);
                sync_ui_from_state(&ui, st);
            });
        });
    });
}

pub fn register_catalog_callbacks(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Navigate callback
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_navigate(move |view| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let view_str = view.to_string();
            ui.set_active_view(view.clone());
            let mut st = state.borrow_mut();
            if !st.is_connected {
                sync_ui_from_state(&ui, &mut st);
                return;
            }
            let lib_ids = st.active_library_ids();

            match view_str.as_str() {
                "listen_now" => {
                    refresh_catalog_for_active_libraries(&ui, &mut st);
                }
                "albums" => {
                    ui.set_is_loading(true);
                    let client = st.client.clone();
                    let sort_type = ui.get_album_sort_type().to_string();
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        let albs = client.get_album_list2(&sort_type, 100, 0, &lib_ids).ok();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if let Some(albs) = albs {
                                    st.current_albums = albs;
                                }
                                ui.set_is_loading(false);
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    });
                }
                "favorites" => {
                    ui.set_is_loading(true);
                    let client = st.client.clone();
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        let starred = client.get_starred2(&lib_ids).ok();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if let Some(starred) = starred {
                                    st.current_songs = starred.song;
                                }
                                ui.set_is_loading(false);
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    });
                }
                "songs" => {
                    ui.set_is_loading(true);
                    let client = st.client.clone();
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        let songs = client.get_random_songs(120, &lib_ids).ok();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if let Some(songs) = songs {
                                    st.current_songs = songs;
                                }
                                ui.set_is_loading(false);
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    });
                }
                _ => {
                    sync_ui_from_state(&ui, &mut st);
                }
            }
        });
    }

    // Multi-Library Toggle (`toggle-library`) — allows selecting one or multiple visible libraries
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_library(move |lib_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let id_str = lib_id.to_string();

            if st.config.selected_library_ids.is_empty() {
                st.config.selected_library_ids =
                    st.libraries.iter().map(|l| l.id.clone()).collect();
            }

            if let Some(pos) = st
                .config
                .selected_library_ids
                .iter()
                .position(|x| x == &id_str)
            {
                if st.config.selected_library_ids.len() > 1 {
                    st.config.selected_library_ids.remove(pos);
                }
            } else {
                st.config.selected_library_ids.push(id_str);
            }

            st.config.save();
            let summary = st.active_libraries_summary();
            ui.set_status_message(format!("Visible libraries updated: {}", summary).into());
            refresh_catalog_for_active_libraries(&ui, &mut st);
        });
    }

    // Select All Libraries
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_all_libraries(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.selected_library_ids = st.libraries.iter().map(|l| l.id.clone()).collect();
            st.config.save();
            ui.set_status_message("All libraries selected".into());
            refresh_catalog_for_active_libraries(&ui, &mut st);
        });
    }

    // Album Sort
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_change_album_sort(move |sort_type| {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_album_sort_type(sort_type.clone());
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            ui.set_is_loading(true);
            let sort_str = sort_type.to_string();
            let lib_ids = st.active_library_ids();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let albs = client.get_album_list2(&sort_str, 100, 0, &lib_ids).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(albs) = albs {
                            st.current_albums = albs;
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Genre selection
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_genre(move |genre_name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let gname = genre_name.to_string();
            ui.set_selected_genre_name(genre_name);
            ui.set_is_loading(true);
            let lib_ids = st.active_library_ids();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let songs = client.get_songs_by_genre(&gname, 100, 0, &lib_ids).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(songs) = songs {
                            st.current_songs = songs;
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Search (`search3` scoped across selected libraries)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_run_search(move |query| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let q = query.to_string();
            if q.trim().is_empty() {
                return;
            }
            ui.set_is_loading(true);
            let lib_ids = st.active_library_ids();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let res = client.search3(&q, &lib_ids).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(res) = res {
                            st.current_songs = res.song;
                            if !res.album.is_empty() {
                                st.current_albums = res.album;
                            }
                            ui.set_status_message(
                                format!("Found {} songs matching '{}'", st.current_songs.len(), q).into(),
                            );
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }
}
