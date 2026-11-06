use super::catalog::refresh_catalog_for_active_libraries;
use super::state::{sync_ui_from_state, with_app_state, AppState};
use crate::api::NavidromeClient;
use crate::*;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub fn register_settings_callbacks(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Set Layout Mode Override ("auto", "mobile", "desktop")
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_layout_override(move |mode| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.layout_override = mode.to_string();
            st.config.save();
            ui.set_layout_override(mode.clone());
            ui.set_status_message(format!("Layout mode set to {}", mode).into());
        });
    }

    // Toggle Nav Section Visibility
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_nav_section(move |section, enabled| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            match section.as_str() {
                "listen_now" => st.config.nav_show_listen_now = enabled,
                "albums" => st.config.nav_show_albums = enabled,
                "artists" => st.config.nav_show_artists = enabled,
                "songs" => st.config.nav_show_songs = enabled,
                "playlists" => st.config.nav_show_playlists = enabled,
                "genres" => st.config.nav_show_genres = enabled,
                "favorites" => st.config.nav_show_favorites = enabled,
                "search" => st.config.nav_show_search = enabled,
                _ => {}
            }
            st.config.save();
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Clear Local Storage (Offline downloads and covers cache)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_clear_local_storage(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.offline.clear_all();
            crate::config::AppConfig::clear_covers_cache();
            st.cover_cache.clear();
            ui.set_status_message("Local storage, offline downloads, and cover cache cleared".into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Select Theme
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_theme(move |theme_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.active_theme = theme_id.to_string();
            st.config.save();
            let theme_def = crate::theme::get_theme(&st.config.active_theme, &st.config.custom_themes);
            crate::theme::apply_theme(&ui, &theme_def);
            ui.set_status_message(format!("Applied theme: {}", theme_def.name).into());
        });
    }

    // Create Custom Theme
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_create_custom_theme(move |name, accent, bg, is_light| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let name_str = name.trim().to_string();
            if name_str.is_empty() {
                return;
            }
            let accent_str = if accent.trim().is_empty() { "#fa2d48".to_string() } else { accent.trim().to_string() };
            let bg_str = if bg.trim().is_empty() {
                if is_light { "#f5f5f7".to_string() } else { "#101014".to_string() }
            } else {
                bg.trim().to_string()
            };
            let id = format!("custom_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
            
            let sidebar_str = if is_light { "#e8e8ed".to_string() } else { "#16161d".to_string() };
            let elevated_str = if is_light { "#ffffff".to_string() } else { "#1d1d26".to_string() };
            let text_p = if is_light { "#1d1d1f".to_string() } else { "#f5f5f7".to_string() };
            let text_s = if is_light { "#6e6e73".to_string() } else { "#9898a4".to_string() };

            let new_theme = crate::config::CustomTheme {
                id: id.clone(),
                name: name_str,
                accent: accent_str,
                bg_canvas: bg_str,
                bg_sidebar: sidebar_str,
                bg_elevated: elevated_str,
                text_primary: text_p,
                text_secondary: text_s,
                is_light,
            };
            st.config.custom_themes.push(new_theme);
            st.config.active_theme = id;
            st.config.save();

            let theme_items = crate::theme::get_all_theme_items(&st.config.custom_themes);
            ui.set_available_themes(slint::ModelRc::new(slint::VecModel::from(theme_items)));
            let theme_def = crate::theme::get_theme(&st.config.active_theme, &st.config.custom_themes);
            crate::theme::apply_theme(&ui, &theme_def);
            ui.set_status_message(format!("Created and applied theme: {}", theme_def.name).into());
        });
    }

    // Delete Custom Theme
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_custom_theme(move |theme_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.custom_themes.retain(|t| t.id != theme_id.as_str());
            if st.config.active_theme == theme_id.as_str() {
                st.config.active_theme = "default".to_string();
            }
            st.config.save();

            let theme_items = crate::theme::get_all_theme_items(&st.config.custom_themes);
            ui.set_available_themes(slint::ModelRc::new(slint::VecModel::from(theme_items)));
            let theme_def = crate::theme::get_theme(&st.config.active_theme, &st.config.custom_themes);
            crate::theme::apply_theme(&ui, &theme_def);
            ui.set_status_message("Deleted custom theme".into());
        });
    }

    // Set Stream Format
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_stream_format(move |fmt| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.stream_format = fmt.to_string();
            st.config.save();
            ui.set_config_stream_format(fmt);
            ui.set_status_message(
                format!("Streaming format set to {}", st.config.stream_format).into(),
            );
        });
    }

    // Set Stream Bitrate
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_stream_bitrate(move |br| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.stream_max_bitrate = br as u32;
            st.config.save();
            ui.set_config_stream_bitrate(br);
            ui.set_status_message(format!("Streaming max bitrate set to {} kbps", br).into());
        });
    }

    // Set Download Format
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_download_format(move |fmt| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.download_format = fmt.to_string();
            st.config.save();
            ui.set_config_download_format(fmt);
            ui.set_status_message(
                format!("Download format set to {}", st.config.download_format).into(),
            );
        });
    }

    // Set Download Bitrate
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_download_bitrate(move |br| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.download_max_bitrate = br as u32;
            st.config.save();
            ui.set_config_download_bitrate(br);
            ui.set_status_message(format!("Download max bitrate set to {} kbps", br).into());
        });
    }

    // Toggle Auto Download Streamed
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_auto_download_streamed(move |enabled| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.auto_download_streamed = enabled;
            st.config.save();
            ui.set_config_auto_download_streamed(enabled);
            ui.set_status_message(
                if enabled {
                    "Auto-download on stream enabled: streamed tracks will be saved offline".into()
                } else {
                    "Auto-download on stream disabled".into()
                },
            );
        });
    }

    // Server Connection & Scan
    {
        let ui_weak = ui.as_weak();
        ui.on_save_and_connect(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_is_loading(true);
            let server_url = ui.get_config_server_url().to_string();
            let username = ui.get_config_username().to_string();
            let password = ui.get_config_password().to_string();
            let ui_weak = ui.as_weak();

            std::thread::spawn(move || {
                let client = NavidromeClient::new(&server_url, &username, &password);
                match client.ping() {
                    Ok(ping_res) => {
                        let folders = client.get_music_folders().ok().unwrap_or_default();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.config.server_url = server_url;
                                st.config.username = username;
                                st.config.password = password;
                                st.config.save();
                                st.client = client;
                                st.is_connected = true;
                                ui.set_is_connected(true);
                                let ver = ping_res
                                    .server_version
                                    .unwrap_or_else(|| ping_res.version.clone());
                                ui.set_server_version_badge(format!("Navidrome {}", ver).into());
                                st.libraries = folders;
                                st.config.selected_library_ids =
                                    st.libraries.iter().map(|l| l.id.clone()).collect();
                                st.config.save();
                                ui.set_active_view("listen_now".into());
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
                                ui.set_status_message(format!("Connection failed: {}", err).into());
                            });
                        });
                    }
                }
            });
        });
    }

    // Disconnect Server
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_disconnect_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.audio.stop();
            st.is_connected = false;
            ui.set_is_playing(false);
            ui.set_is_connected(false);
            ui.set_status_message("".into());
        });
    }

    // Trigger Server Scan
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_trigger_server_scan(move |full_scan| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                ui.set_status_message("Connect to Navidrome server first".into());
                return;
            }
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            ui.set_status_message(
                if full_scan {
                    "Requesting Navidrome full media rescan...".into()
                } else {
                    "Requesting Navidrome quick scan...".into()
                },
            );
            std::thread::spawn(move || {
                if let Ok(scan) = client.start_scan(full_scan) {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        ui.set_scan_in_progress(scan.scanning);
                        ui.set_scan_count(scan.count.unwrap_or(0) as i32);
                        ui.set_status_message(
                            if full_scan {
                                "Started Navidrome full media rescan".into()
                            } else {
                                "Started Navidrome quick scan".into()
                            },
                        );
                    });
                }
            });
        });
    }
}
