use super::state::{sync_ui_from_state, with_app_state, AppState};
use crate::models::format_duration;
use crate::*;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

pub fn play_track_by_id(ui: &AppWindow, state: &mut AppState, song_id: &str) {
    let song = match state.find_song_by_id(song_id) {
        Some(s) => s,
        None => {
            ui.set_status_message("Track metadata not found".into());
            return;
        }
    };

    if state.queue.is_empty() {
        state.queue = if !state.current_songs.is_empty() {
            state.current_songs.clone()
        } else {
            vec![song.clone()]
        };
    } else if !state.queue.iter().any(|q| q.id == song.id) {
        state.queue.push(song.clone());
    }

    let duration_secs = song.duration.unwrap_or(0).max(1);

    // Update Now Playing UI immediately
    let cover_opt = state.resolve_cover(song.cover_art.as_deref());
    ui.set_now_playing_cover(cover_opt.clone().unwrap_or_default());
    ui.set_now_playing_has_cover(cover_opt.is_some());
    ui.set_now_playing_id(song.id.clone().into());
    ui.set_now_playing_title(song.title.clone().into());
    ui.set_now_playing_artist(song.display_artist_name().into());
    ui.set_now_playing_album(song.display_album_name().into());
    ui.set_now_playing_album_id(song.album_id.clone().unwrap_or_default().into());
    ui.set_now_playing_artist_id(song.artist_id.clone().unwrap_or_default().into());
    ui.set_now_playing_starred(song.starred.is_some());
    ui.set_now_playing_rating(song.user_rating.unwrap_or(0) as i32);
    ui.set_now_playing_is_local(state.offline.is_downloaded(&song.id));
    ui.set_is_playing(true);
    ui.set_playback_current_time("0:00".into());
    ui.set_playback_total_time(format_duration(duration_secs).into());
    ui.set_playback_progress(0.0);

    state.play_generation = state.play_generation.wrapping_add(1);
    let my_gen = state.play_generation;

    // 1. Play from local offline storage if already downloaded
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(local_path) = state.offline.local_file_path(&song.id) {
        match state.audio.play_file(&song.id, &local_path, duration_secs) {
            Ok(()) => {
                ui.set_now_playing_is_local(true);
                ui.set_now_playing_quality(format!("LOCAL · {}", song.quality_badge()).into());
                ui.set_status_message(format!("Playing '{}' from local offline storage", song.title).into());
                sync_ui_from_state(ui, state);

                if state.is_connected {
                    let client = state.client.clone();
                    let sid = song.id.clone();
                    let s_artist = song.display_artist_name();
                    let s_title = song.title.clone();
                    let scrobble_enabled = state.config.scrobble_enabled;
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        if scrobble_enabled {
                            let _ = client.scrobble(&sid, false);
                        }
                        let lyrics = client.get_lyrics_for_song(&sid, &s_artist, &s_title);
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if st.play_generation != my_gen { return; }
                                st.current_lyrics = lyrics;
                                let lyric_items: Vec<LyricLineItem> = st
                                    .current_lyrics
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, l)| LyricLineItem {
                                        time_ms: l.start.unwrap_or(0) as i32,
                                        text: l.value.clone().into(),
                                        active: idx == 0,
                                    })
                                    .collect();
                                ui.set_lyrics_lines(ModelRc::new(VecModel::from(lyric_items)));
                            });
                        });
                    });
                }
                return;
            }
            Err(err) => {
                ui.set_status_message(format!("Local file error ({}), streaming instead...", err).into());
            }
        }
    }

    // 2. Otherwise stream from Navidrome in background
    if !state.is_connected {
        ui.set_is_playing(false);
        ui.set_status_message(
            "Not connected to Navidrome server (and track is not downloaded offline)".into(),
        );
        return;
    }

    ui.set_now_playing_quality("BUFFERING...".into());
    ui.set_status_message(format!("Buffering '{}'...", song.title).into());
    sync_ui_from_state(ui, state);

    let client = state.client.clone();
    let stream_format = state.config.stream_format.clone();
    let stream_max_bitrate = state.config.stream_max_bitrate;
    let auto_download = state.config.auto_download_streamed && !state.offline.is_downloaded(&song.id);
    let download_format = state.config.download_format.clone();
    let download_max_bitrate = state.config.download_max_bitrate;
    let scrobble_enabled = state.config.scrobble_enabled;
    let song_clone = song.clone();
    let ui_weak = ui.as_weak();

    std::thread::spawn(move || {
        let stream_bytes_res = client.fetch_stream_bytes(
            &song_clone.id,
            &stream_format,
            stream_max_bitrate,
        );

        let dl_bytes = if auto_download && stream_bytes_res.is_ok() {
            if download_format != stream_format || download_max_bitrate != stream_max_bitrate {
                client.fetch_download_bytes(&song_clone.id, &download_format, download_max_bitrate).ok()
            } else {
                stream_bytes_res.as_ref().ok().cloned()
            }
        } else {
            None
        };

        if scrobble_enabled {
            let _ = client.scrobble(&song_clone.id, false);
        }

        let lyrics = client.get_lyrics_for_song(
            &song_clone.id,
            &song_clone.display_artist_name(),
            &song_clone.title,
        );

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            with_app_state(move |state| {
                if state.play_generation != my_gen {
                    return;
                }

                if let Some(dl_data) = dl_bytes {
                    let _ = state.offline.store_track(song_clone.clone(), &dl_data, &download_format, download_max_bitrate);
                }

                state.current_lyrics = lyrics;
                let lyric_items: Vec<LyricLineItem> = state
                    .current_lyrics
                    .iter()
                    .enumerate()
                    .map(|(idx, l)| LyricLineItem {
                        time_ms: l.start.unwrap_or(0) as i32,
                        text: l.value.clone().into(),
                        active: idx == 0,
                    })
                    .collect();
                ui.set_lyrics_lines(ModelRc::new(VecModel::from(lyric_items)));

                match stream_bytes_res {
                    Ok(stream_bytes) => {
                        if let Err(err) = state.audio.play_bytes(&song_clone.id, stream_bytes, duration_secs) {
                            ui.set_status_message(format!("Playback decode error: {}", err).into());
                            ui.set_is_playing(false);
                            return;
                        }

                        let stream_badge = if stream_format == "raw" && stream_max_bitrate == 0 {
                            song_clone.quality_badge()
                        } else if stream_max_bitrate > 0 {
                            format!("{} {}k", stream_format.to_ascii_uppercase(), stream_max_bitrate)
                        } else {
                            stream_format.to_ascii_uppercase()
                        };
                        ui.set_now_playing_quality(stream_badge.into());
                        ui.set_is_playing(true);
                        if auto_download {
                            ui.set_status_message(format!("Streaming & auto-downloaded '{}'", song_clone.title).into());
                        } else {
                            ui.set_status_message(format!("Streaming '{}'", song_clone.title).into());
                        }
                    }
                    Err(err) => {
                        ui.set_status_message(format!("Stream error: {}", err).into());
                        ui.set_is_playing(false);
                    }
                }

                sync_ui_from_state(&ui, state);
            });
        });
    });
}

pub fn advance_track(ui: &AppWindow, state: &mut AppState, forward: bool) {
    if state.queue.is_empty() {
        return;
    }
    let current_id = state.audio.current_song_id.clone().unwrap_or_default();
    if state.repeat_mode == "one" && forward && !current_id.is_empty() {
        play_track_by_id(ui, state, &current_id);
        return;
    }

    let current_idx = state
        .queue
        .iter()
        .position(|s| s.id == current_id)
        .unwrap_or(0);

    let next_idx = if state.shuffle {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(1);
        (current_idx + 1 + nanos) % state.queue.len()
    } else if forward {
        if current_idx + 1 < state.queue.len() {
            current_idx + 1
        } else if state.repeat_mode == "all" {
            0
        } else {
            state.audio.stop();
            ui.set_is_playing(false);
            return;
        }
    } else if current_idx > 0 {
        current_idx - 1
    } else {
        0
    };

    if let Some(next_song) = state.queue.get(next_idx).cloned() {
        play_track_by_id(ui, state, &next_song.id);
    }
}

pub fn register_playback_callbacks(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Play Song
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_play_song(move |song_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            play_track_by_id(&ui, &mut st, &song_id);
        });
    }

    // Add to Queue
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_queue_song(move |song_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if let Some(s) = st.find_song_by_id(&song_id) {
                st.queue.push(s.clone());
                ui.set_status_message(format!("Added '{}' to queue", s.title).into());
                sync_ui_from_state(&ui, &mut st);
            }
        });
    }

    // Transport Play/Pause
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_play_pause(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if st.audio.current_song_id.is_none() {
                if let Some(first) = st.current_songs.first().cloned() {
                    play_track_by_id(&ui, &mut st, &first.id);
                    return;
                }
            }
            let playing = st.audio.toggle_play_pause();
            ui.set_is_playing(playing);
        });
    }

    // Transport Next
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_next(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            advance_track(&ui, &mut st, true);
        });
    }

    // Transport Prev
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_prev(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            advance_track(&ui, &mut st, false);
        });
    }

    // Transport Seek
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_seek(move |val| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.audio.seek_ratio(val);
            let elapsed = st.audio.elapsed_secs() as u32;
            ui.set_playback_current_time(format_duration(elapsed).into());
        });
    }

    // Transport Volume
    {
        let state = state.clone();
        ui.on_transport_volume(move |vol| {
            let mut st = state.borrow_mut();
            st.audio.set_volume(vol);
            st.config.volume = vol;
            st.config.save();
        });
    }

    // Toggle Shuffle
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_shuffle(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.shuffle = !st.shuffle;
            ui.set_shuffle_enabled(st.shuffle);
        });
    }

    // Cycle Repeat
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_cycle_repeat(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.repeat_mode = match st.repeat_mode.as_str() {
                "off" => "all".to_string(),
                "all" => "one".to_string(),
                _ => "off".to_string(),
            };
            ui.set_repeat_mode(st.repeat_mode.clone().into());
        });
    }
}
