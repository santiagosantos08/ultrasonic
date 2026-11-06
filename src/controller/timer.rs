use super::playback::advance_track;
use super::state::{sync_ui_from_state, AppState};
use crate::models::format_duration;
use crate::*;
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub fn start_playback_timer(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    let timer = Timer::default();
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        timer.start(
            TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let mut st = state.borrow_mut();

                // 1. Shimmer animation pulse toggle for skeleton loaders
                ui.set_pulse_phase(!ui.get_pulse_phase());

                // 2. Progressive cover art loading: when new covers finish downloading in background, update models
                if st.has_new_covers {
                    st.has_new_covers = false;
                    sync_ui_from_state(&ui, &mut st);
                }

                if !st.audio.is_playing {
                    return;
                }

                let elapsed = st.audio.elapsed_secs();
                let total = st.audio.duration_secs.max(1) as f32;
                let ratio = (elapsed / total).clamp(0.0, 1.0);

                ui.set_playback_progress(ratio);
                ui.set_playback_current_time(format_duration(elapsed as u32).into());

                // Highlight active synchronized lyric line
                let elapsed_ms = (elapsed * 1000.0) as i64;
                if !st.current_lyrics.is_empty() {
                    let mut active_idx = 0usize;
                    for (idx, line) in st.current_lyrics.iter().enumerate() {
                        if let Some(start) = line.start {
                            if elapsed_ms >= start {
                                active_idx = idx;
                            }
                        }
                    }
                    let items: Vec<LyricLineItem> = st
                        .current_lyrics
                        .iter()
                        .enumerate()
                        .map(|(idx, l)| LyricLineItem {
                            time_ms: l.start.unwrap_or(0) as i32,
                            text: l.value.clone().into(),
                            active: idx == active_idx,
                        })
                        .collect();
                    ui.set_lyrics_lines(ModelRc::new(VecModel::from(items)));
                }

                // Navidrome requires `scrobble(submission=true)` to mark a track as played
                if ratio >= 0.5 && !st.audio.scrobble_submitted_for_current {
                    st.audio.scrobble_submitted_for_current = true;
                    if st.is_connected && st.config.scrobble_enabled {
                        if let Some(ref sid) = st.audio.current_song_id {
                            let client = st.client.clone();
                            let sid_c = sid.clone();
                            std::thread::spawn(move || {
                                let _ = client.scrobble(&sid_c, true);
                            });
                        }
                    }
                }

                if st.audio.is_track_finished() {
                    advance_track(&ui, &mut st, true);
                }
            },
        );
    }
    std::mem::forget(timer);
}
