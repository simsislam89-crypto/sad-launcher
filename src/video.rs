//! YouTube inside the launcher: the official embed player (YouTube IFrame
//! API) in a WebView2 child window laid over the viewer's media rect.
//!
//! Why this and not a hand-made player: YouTube streams need its own
//! player to stay legal (no stream extraction), and WebView2 is the Edge
//! engine already in Windows 11 — hardware decoding, adaptive quality up to
//! 4K, starts as fast as in the browser. The window costs ~100 МБ while it
//! lives, so it exists only while a card with videos (or the viewer) is open.
//!
//! The page is served by a custom protocol (`http://sad.player/` on
//! Windows): the embed then gets a real origin/referrer — YouTube refuses
//! players without one («Error 153»).

#![cfg(windows)]

use anyhow::{anyhow, Result};
use wry::raw_window_handle::HasWindowHandle;
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::{Rect, WebView, WebViewBuilder};

/// What the player reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerEvent {
    Ready,
    Playing,
    Paused,
    Ended,
    Buffering,
    /// YouTube error code: 2 bad id, 5 HTML5, 100 removed, 101/150 the
    /// owner forbids playback on other sites.
    Error(u32),
}

impl PlayerEvent {
    pub fn parse(msg: &str) -> Option<PlayerEvent> {
        if msg == "ready" {
            return Some(PlayerEvent::Ready);
        }
        if let Some(code) = msg.strip_prefix("error:") {
            return code.trim().parse().ok().map(PlayerEvent::Error);
        }
        match msg.strip_prefix("state:")?.trim() {
            "1" => Some(PlayerEvent::Playing),
            "2" => Some(PlayerEvent::Paused),
            "0" => Some(PlayerEvent::Ended),
            "3" => Some(PlayerEvent::Buffering),
            _ => None,
        }
    }
}

/// Human text for a player error.
pub fn error_text(code: u32) -> &'static str {
    match code {
        101 | 150 | 153 => "Автор запретил смотреть это видео вне YouTube",
        100 => "Видео удалено или скрыто",
        2 => "Неверная ссылка на видео",
        _ => "Видео не запустилось",
    }
}

pub struct VideoView {
    webview: WebView,
    events: Arc<Mutex<Vec<PlayerEvent>>>,
    /// Video id the player holds (cued or playing).
    pub current: String,
    bounds: (i32, i32, u32, u32),
    visible: bool,
}

/// `[A-Za-z0-9_-]{11}` — the only ids that go into the page script.
pub fn valid_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn page(first: &str) -> String {
    const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="referrer" content="strict-origin-when-cross-origin">
<style>html,body{margin:0;height:100%;background:#000;overflow:hidden}#p{position:fixed;inset:0;width:100%;height:100%}</style>
</head><body><div id="p"></div>
<script>
var player=null, pending="__FIRST__", autoplay=false;
function send(m){try{window.ipc.postMessage(m)}catch(e){}}
function onYouTubeIframeAPIReady(){
  player=new YT.Player('p',{host:'https://www.youtube-nocookie.com',videoId:pending,
    playerVars:{playsinline:1,rel:0,modestbranding:1,iv_load_policy:3,fs:1,origin:location.origin},
    events:{onReady:function(e){send('ready'); if(autoplay){e.target.playVideo()}},
            onStateChange:function(e){send('state:'+e.data)},
            onError:function(e){send('error:'+e.data)}}});
}
function play(id){autoplay=true; if(player&&player.loadVideoById){ if(player.getVideoData&&player.getVideoData().video_id===id){player.playVideo()} else {player.loadVideoById({videoId:id,suggestedQuality:'highres'})} } else {pending=id}}
function pause(){autoplay=false; if(player&&player.pauseVideo){player.pauseVideo()}}
</script>
<script src="https://www.youtube.com/iframe_api"></script>
</body></html>"#;
    PAGE.replace("__FIRST__", first)
}

impl VideoView {
    /// Creates the (hidden) player, already loading the first video so a
    /// click starts playback at once.
    pub fn new(window: &impl HasWindowHandle, first: &str) -> Result<VideoView> {
        if !valid_id(first) {
            return Err(anyhow!("Неверный id видео"));
        }
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let html = page(first);
        let data_dir = crate::config::data_dir().join("webview");
        let mut context = wry::WebContext::new(Some(data_dir));
        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_custom_protocol("sad".into(), move |_id, _req| {
                http_response(html.clone().into_bytes())
            })
            .with_url("sad://player/")
            .with_autoplay(true)
            .with_visible(false)
            .with_background_color((0, 0, 0, 255))
            .with_devtools(false)
            .with_bounds(Rect {
                position: PhysicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(16u32, 9u32).into(),
            })
            // Links inside the player (title, logo, «смотреть на YouTube»)
            // open in the browser instead of replacing the player page.
            .with_new_window_req_handler(|url, _features| {
                if url.starts_with("https://") {
                    let _ = open::that_detached(&url);
                }
                wry::NewWindowResponse::Deny
            })
            .with_ipc_handler(move |req| {
                if let Some(ev) = PlayerEvent::parse(req.body()) {
                    if let Ok(mut list) = sink.lock() {
                        list.push(ev);
                    }
                }
            })
            .build_as_child(window)
            .map_err(|e| anyhow!("WebView2 не запустился: {e}"))?;
        Ok(VideoView { webview, events, current: first.to_string(), bounds: (0, 0, 16, 9), visible: false })
    }

    /// Places the player over `(x, y, w, h)` physical pixels of the window.
    pub fn place(&mut self, bounds: (i32, i32, u32, u32)) {
        if bounds != self.bounds {
            self.bounds = bounds;
            let _ = self.webview.set_bounds(Rect {
                position: PhysicalPosition::new(bounds.0, bounds.1).into(),
                size: PhysicalSize::new(bounds.2.max(1), bounds.3.max(1)).into(),
            });
        }
    }

    pub fn show(&mut self, visible: bool) {
        if visible != self.visible {
            self.visible = visible;
            let _ = self.webview.set_visible(visible);
            if !visible {
                let _ = self.webview.evaluate_script("pause()");
                let _ = self.webview.focus_parent();
            }
        }
    }

    pub fn play(&mut self, id: &str) {
        if valid_id(id) {
            self.current = id.to_string();
            let _ = self.webview.evaluate_script(&format!("play('{id}')"));
        }
    }

    pub fn take_events(&self) -> Vec<PlayerEvent> {
        self.events.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default()
    }
}

fn http_response(body: Vec<u8>) -> wry::http::Response<Cow<'static, [u8]>> {
    wry::http::Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .body(Cow::Owned(body))
        .unwrap_or_else(|_| wry::http::Response::new(Cow::Borrowed(&b""[..])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_messages_parse() {
        assert_eq!(PlayerEvent::parse("ready"), Some(PlayerEvent::Ready));
        assert_eq!(PlayerEvent::parse("state:1"), Some(PlayerEvent::Playing));
        assert_eq!(PlayerEvent::parse("state:0"), Some(PlayerEvent::Ended));
        assert_eq!(PlayerEvent::parse("error:150"), Some(PlayerEvent::Error(150)));
        assert_eq!(PlayerEvent::parse("state:-1"), None);
        assert_eq!(PlayerEvent::parse("<script>"), None);
    }

    #[test]
    fn only_real_ids_reach_the_script() {
        assert!(valid_id("dQw4w9WgXcQ"));
        assert!(!valid_id("dQw4w9WgXc'"));
        assert!(!valid_id("short"));
        assert!(page("dQw4w9WgXcQ").contains("pending=\"dQw4w9WgXcQ\""));
    }
}
