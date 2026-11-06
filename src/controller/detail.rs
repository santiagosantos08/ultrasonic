use super::playback::play_track_by_id;
use super::state::{sync_ui_from_state, with_app_state, AppState};
use crate::*;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub fn register_detail_callbacks(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Open Album Detail
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_open_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            ui.set_is_loading(true);
            ui.set_active_view("album_detail".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let alb = client.get_album(&aid).ok();
                let notes = client
                    .get_album_info2(&aid)
                    .ok()
                    .and_then(|info| info.notes)
                    .unwrap_or_default();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(ref alb) = alb {
                            let slint_alb = st.to_slint_album(alb);
                            ui.set_current_album_detail(slint_alb);
                            st.current_album_songs = alb.song.clone();
                        }
                        ui.set_current_album_notes(notes.into());
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Play Album
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_play_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            if ui.get_current_album_detail().id == album_id && !st.current_album_songs.is_empty() {
                st.queue = st.current_album_songs.clone();
                let first_id = st.current_album_songs[0].id.clone();
                play_track_by_id(&ui, &mut st, &first_id);
                return;
            }
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                if let Ok(alb) = client.get_album(&aid) {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        with_app_state(move |st| {
                            if !alb.song.is_empty() {
                                st.queue = alb.song.clone();
                                let first_id = alb.song[0].id.clone();
                                play_track_by_id(&ui, st, &first_id);
                            }
                        });
                    });
                }
            });
        });
    }

    // Open Artist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_open_artist(move |artist_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let arid = artist_id.to_string();
            ui.set_selected_artist_id(arid.clone().into());
            ui.set_is_loading(true);
            ui.set_active_view("artists".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let detail = client.get_artist(&arid).ok();
                let top = if let Some(ref d) = detail {
                    client.get_top_songs(&d.name, 30).ok()
                } else {
                    None
                };
                let bio = client
                    .get_artist_info2(&arid)
                    .ok()
                    .and_then(|info| info.biography)
                    .unwrap_or_default();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(detail) = detail {
                            ui.set_selected_artist_name(detail.name.clone().into());
                            st.selected_artist_albums = detail.album;
                        }
                        if let Some(top) = top {
                            st.selected_artist_songs = top;
                        }
                        ui.set_selected_artist_bio(bio.into());
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Toggle Star (Song)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_star(move |song_id, star| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let sid = song_id.to_string();
            if st.is_connected {
                let client = st.client.clone();
                let sid_c = sid.clone();
                std::thread::spawn(move || {
                    let _ = client.set_star(&sid_c, false, star);
                });
            }
            let marker = if star {
                Some("starred".to_string())
            } else {
                None
            };
            for s in st.current_songs.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            for s in st.current_album_songs.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            for s in st.current_playlist_songs.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            for s in st.queue.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            if ui.get_now_playing_id() == song_id {
                ui.set_now_playing_starred(star);
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Toggle Star (Album)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_album_star(move |album_id, star| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let aid = album_id.to_string();
            if st.is_connected {
                let client = st.client.clone();
                let aid_c = aid.clone();
                std::thread::spawn(move || {
                    let _ = client.set_star(&aid_c, true, star);
                });
            }
            let mut detail = ui.get_current_album_detail();
            if detail.id == album_id {
                detail.starred = star;
                ui.set_current_album_detail(detail);
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Set 1-5 Star Rating (`setRating`)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_song_rating(move |song_id, rating| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let sid = song_id.to_string();
            let r = (rating as u32).min(5);
            if st.is_connected {
                let client = st.client.clone();
                let sid_c = sid.clone();
                std::thread::spawn(move || {
                    let _ = client.set_rating(&sid_c, r);
                });
            }
            for s in st.current_songs.iter_mut() {
                if s.id == sid {
                    s.user_rating = Some(r);
                }
            }
            for s in st.queue.iter_mut() {
                if s.id == sid {
                    s.user_rating = Some(r);
                }
            }
            if ui.get_now_playing_id() == song_id {
                ui.set_now_playing_rating(rating);
            }
            ui.set_status_message(format!("Rated {} stars", rating).into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Shares (`createShare` / `deleteShare`)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_share_current_album(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let alb = ui.get_current_album_detail();
            let aid = alb.id.to_string();
            let desc = format!("{} — {}", alb.title, alb.artist);
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                if let Ok(mut new_shares) = client.create_share(&aid, &desc) {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        with_app_state(move |st| {
                            st.current_shares.append(&mut new_shares);
                            ui.set_status_message(format!("Created Navidrome share link for '{}'", desc).into());
                            sync_ui_from_state(&ui, st);
                        });
                    });
                }
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_share(move |share_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let sid = share_id.to_string();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.delete_share(&sid);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        st.current_shares.retain(|s| s.id != sid);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }
}
