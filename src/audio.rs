use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct AudioEngine {
    #[cfg(not(target_arch = "wasm32"))]
    _stream: Option<rodio::OutputStream>,
    #[cfg(not(target_arch = "wasm32"))]
    stream_handle: Option<rodio::OutputStreamHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    sink: Option<rodio::Sink>,

    #[cfg(target_arch = "wasm32")]
    html_audio: Option<web_sys::HtmlAudioElement>,

    pub is_playing: bool,
    pub duration_secs: u32,
    pub volume: f32,
    started_at: Option<Instant>,
    accumulated_secs: f32,
    pub current_song_id: Option<String>,
    pub scrobble_submitted_for_current: bool,
}

impl AudioEngine {
    pub fn new(initial_volume: f32) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let (stream, handle) = match rodio::OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(_) => (None, None),
        };

        #[cfg(target_arch = "wasm32")]
        let html_audio = web_sys::HtmlAudioElement::new().ok();

        Self {
            #[cfg(not(target_arch = "wasm32"))]
            _stream: stream,
            #[cfg(not(target_arch = "wasm32"))]
            stream_handle: handle,
            #[cfg(not(target_arch = "wasm32"))]
            sink: None,

            #[cfg(target_arch = "wasm32")]
            html_audio,

            is_playing: false,
            duration_secs: 0,
            volume: initial_volume.clamp(0.0, 1.0),
            started_at: None,
            accumulated_secs: 0.0,
            current_song_id: None,
            scrobble_submitted_for_current: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn play_file(&mut self, song_id: &str, path: &Path, duration_secs: u32) -> Result<(), String> {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("Failed opening local audio file {}: {}", path.display(), e))?;
        let reader = std::io::BufReader::new(file);

        self.stop();
        let handle = self
            .stream_handle
            .as_ref()
            .ok_or_else(|| "No audio output device available".to_string())?;
        let sink = rodio::Sink::try_new(handle)
            .map_err(|e| format!("Failed creating audio sink: {}", e))?;
        sink.set_volume(self.volume);
        let source = rodio::Decoder::new(reader)
            .map_err(|e| format!("Audio decode error: {}", e))?;
        sink.append(source);
        sink.play();

        self.sink = Some(sink);
        self.current_song_id = Some(song_id.to_string());
        self.duration_secs = duration_secs.max(1);
        self.accumulated_secs = 0.0;
        self.scrobble_submitted_for_current = false;
        self.is_playing = true;
        self.started_at = Some(Instant::now());
        Ok(())
    }

    pub fn play_bytes(&mut self, song_id: &str, bytes: Vec<u8>, duration_secs: u32) -> Result<(), String> {
        self.stop();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let handle = self
                .stream_handle
                .as_ref()
                .ok_or_else(|| "No audio output device available".to_string())?;
            let sink = rodio::Sink::try_new(handle)
                .map_err(|e| format!("Failed creating audio sink: {}", e))?;
            sink.set_volume(self.volume);
            let cursor = Cursor::new(bytes);
            let source = rodio::Decoder::new(cursor)
                .map_err(|e| format!("Audio decode error: {}", e))?;
            sink.append(source);
            sink.play();
            self.sink = Some(sink);
        }

        self.current_song_id = Some(song_id.to_string());
        self.duration_secs = duration_secs.max(1);
        self.accumulated_secs = 0.0;
        self.scrobble_submitted_for_current = false;
        self.is_playing = true;
        self.started_at = Some(Instant::now());
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn play_url_wasm(&mut self, song_id: &str, url: &str, duration_secs: u32) {
        self.stop();
        self.current_song_id = Some(song_id.to_string());
        self.duration_secs = duration_secs.max(1);
        self.accumulated_secs = 0.0;
        self.scrobble_submitted_for_current = false;
        if let Some(ref audio) = self.html_audio {
            audio.set_src(url);
            audio.set_volume(self.volume as f64);
            let _ = audio.play();
        }
        self.is_playing = true;
        self.started_at = Some(Instant::now());
    }

    pub fn toggle_play_pause(&mut self) -> bool {
        if self.is_playing {
            self.pause();
        } else {
            self.resume();
        }
        self.is_playing
    }

    pub fn pause(&mut self) {
        if self.is_playing {
            if let Some(started) = self.started_at.take() {
                self.accumulated_secs += started.elapsed().as_secs_f32();
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref sink) = self.sink {
                sink.pause();
            }
            #[cfg(target_arch = "wasm32")]
            if let Some(ref audio) = self.html_audio {
                let _ = audio.pause();
            }
            self.is_playing = false;
        }
    }

    pub fn resume(&mut self) {
        if !self.is_playing && self.current_song_id.is_some() {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref sink) = self.sink {
                sink.play();
            }
            #[cfg(target_arch = "wasm32")]
            if let Some(ref audio) = self.html_audio {
                let _ = audio.play();
            }
            self.started_at = Some(Instant::now());
            self.is_playing = true;
        }
    }

    pub fn stop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(ref audio) = self.html_audio {
            let _ = audio.pause();
        }
        self.is_playing = false;
        self.started_at = None;
        self.accumulated_secs = 0.0;
    }

    pub fn seek_ratio(&mut self, ratio: f32) {
        let clamped = ratio.clamp(0.0, 1.0);
        let target_secs = clamped * (self.duration_secs as f32);
        self.accumulated_secs = target_secs;
        if self.is_playing {
            self.started_at = Some(Instant::now());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref sink) = self.sink {
            let _ = sink.try_seek(Duration::from_secs_f32(target_secs));
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(ref audio) = self.html_audio {
            audio.set_current_time(target_secs as f64);
        }
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref sink) = self.sink {
            sink.set_volume(self.volume);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(ref audio) = self.html_audio {
            audio.set_volume(self.volume as f64);
        }
    }

    pub fn elapsed_secs(&self) -> f32 {
        let live = if self.is_playing {
            self.started_at
                .map(|s| s.elapsed().as_secs_f32())
                .unwrap_or(0.0)
        } else {
            0.0
        };
        (self.accumulated_secs + live).min(self.duration_secs as f32)
    }

    pub fn is_track_finished(&self) -> bool {
        if !self.is_playing || self.current_song_id.is_none() {
            return false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref sink) = self.sink {
            return sink.empty();
        }
        self.duration_secs > 0 && self.elapsed_secs() >= (self.duration_secs as f32)
    }
}
