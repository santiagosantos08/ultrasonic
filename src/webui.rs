#[cfg(not(target_arch = "wasm32"))]
use crate::api::NavidromeClient;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::{AppConfig, OfflineIndex};
#[cfg(not(target_arch = "wasm32"))]
use crate::models::format_duration;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::json;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

#[cfg(not(target_arch = "wasm32"))]
const WEBUI_HTML: &str = include_str!("../web/index.html");

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_webui_server(port: u16) {
    thread::spawn(move || {
        let bind_addr = format!("127.0.0.1:{}", port);
        let server = match tiny_http::Server::http(&bind_addr) {
            Ok(s) => s,
            Err(_) => return,
        };

        for mut request in server.incoming_requests() {
            let url = request.url().to_string();
            if url == "/" || url.starts_with("/index.html") {
                let header = tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                )
                .unwrap();
                let _ = request.respond(
                    tiny_http::Response::from_string(WEBUI_HTML).with_header(header),
                );
            } else if url.starts_with("/api/state") {
                let mut cfg = AppConfig::load();
                let offline = OfflineIndex::load();
                let client = NavidromeClient::new(&cfg.server_url, &cfg.username, &cfg.password);

                let mut connected = false;
                let mut libraries_vec = Vec::new();
                let mut songs_vec = Vec::new();

                if client.is_configured() && client.ping().is_ok() {
                    connected = true;
                    if let Ok(folders) = client.get_music_folders() {
                        if cfg.selected_library_ids.is_empty() {
                            cfg.selected_library_ids = folders.iter().map(|f| f.id.clone()).collect();
                            cfg.save();
                        }
                        libraries_vec = folders;
                    }
                    if let Ok(songs) = client.get_random_songs(100, &cfg.selected_library_ids) {
                        songs_vec = songs;
                    }
                }

                let libs_json: Vec<_> = libraries_vec
                    .iter()
                    .map(|l| json!({ "id": l.id, "name": l.name }))
                    .collect();

                let songs_json: Vec<_> = songs_vec
                    .iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "title": s.title,
                            "artist": s.display_artist_name(),
                            "album": s.display_album_name(),
                            "quality": s.quality_badge(),
                            "starred": s.starred.is_some(),
                            "is_downloaded": offline.is_downloaded(&s.id),
                            "duration": format_duration(s.duration.unwrap_or(0)),
                        })
                    })
                    .collect();

                let body = json!({
                    "connected": connected,
                    "server_url": cfg.server_url,
                    "username": cfg.username,
                    "libraries": libs_json,
                    "selected_libraries": cfg.selected_library_ids,
                    "songs": songs_json,
                    "auto_download": cfg.auto_download_streamed,
                    "stream_format": cfg.stream_format,
                    "stream_bitrate": cfg.stream_max_bitrate,
                    "download_format": cfg.download_format,
                    "download_bitrate": cfg.download_max_bitrate,
                })
                .to_string();

                let header = tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"application/json"[..],
                )
                .unwrap();
                let _ = request
                    .respond(tiny_http::Response::from_string(body).with_header(header));
            } else if url.starts_with("/api/connect") {
                let mut body_str = String::new();
                let _ = request.as_reader().read_to_string(&mut body_str);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    let mut cfg = AppConfig::load();
                    if let Some(u) = val.get("server_url").and_then(|v| v.as_str()) {
                        cfg.server_url = u.to_string();
                    }
                    if let Some(u) = val.get("username").and_then(|v| v.as_str()) {
                        cfg.username = u.to_string();
                    }
                    if let Some(p) = val.get("password").and_then(|v| v.as_str()) {
                        cfg.password = p.to_string();
                    }
                    cfg.selected_library_ids.clear();
                    cfg.save();
                }
                let _ = request.respond(tiny_http::Response::from_string("{\"ok\":true}"));
            } else if let Some(lib_id) = url.strip_prefix("/api/toggle-library/") {
                let decoded_id = urlencoding::decode(lib_id)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| lib_id.to_string());
                let mut cfg = AppConfig::load();
                if let Some(pos) = cfg.selected_library_ids.iter().position(|x| x == &decoded_id) {
                    if cfg.selected_library_ids.len() > 1 {
                        cfg.selected_library_ids.remove(pos);
                    }
                } else {
                    cfg.selected_library_ids.push(decoded_id);
                }
                cfg.save();
                let _ = request.respond(tiny_http::Response::from_string("{\"ok\":true}"));
            } else if url.starts_with("/api/toggle-autodl") {
                let mut cfg = AppConfig::load();
                cfg.auto_download_streamed = !cfg.auto_download_streamed;
                cfg.save();
                let _ = request.respond(tiny_http::Response::from_string("{\"ok\":true}"));
            } else if let Some(song_id) = url.strip_prefix("/api/stream/") {
                let decoded_id = urlencoding::decode(song_id)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| song_id.to_string());
                let cfg = AppConfig::load();
                let mut offline = OfflineIndex::load();

                // 1. Serve from local offline storage if already downloaded
                let audio_bytes_opt = if let Some(path) = offline.local_file_path(&decoded_id) {
                    std::fs::read(path).ok()
                } else {
                    // 2. Stream from Navidrome server
                    let client = NavidromeClient::new(&cfg.server_url, &cfg.username, &cfg.password);
                    if client.is_configured() {
                        if let Ok(bytes) = client.fetch_stream_bytes(
                            &decoded_id,
                            &cfg.stream_format,
                            cfg.stream_max_bitrate,
                        ) {
                            if cfg.auto_download_streamed && !offline.is_downloaded(&decoded_id) {
                                if let Ok(random_songs) = client.get_random_songs(200, &cfg.selected_library_ids) {
                                    if let Some(song) = random_songs.into_iter().find(|s| s.id == decoded_id) {
                                        let _ = offline.store_track(
                                            song,
                                            &bytes,
                                            &cfg.download_format,
                                            cfg.download_max_bitrate,
                                        );
                                    }
                                }
                            }
                            Some(bytes)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                match audio_bytes_opt {
                    Some(audio_bytes) => {
                        let header = tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"audio/mpeg"[..],
                        )
                        .unwrap();
                        let _ = request.respond(
                            tiny_http::Response::from_data(audio_bytes).with_header(header),
                        );
                    }
                    None => {
                        let _ = request.respond(
                            tiny_http::Response::from_string("Stream unavailable — check Navidrome connection")
                                .with_status_code(502),
                        );
                    }
                }
            } else {
                let _ = request.respond(
                    tiny_http::Response::from_string("Not Found").with_status_code(404),
                );
            }
        }
    });
}
