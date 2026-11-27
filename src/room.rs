//! Rooms: friends meet by a 5-digit code. Presence travels through the
//! public ntfy.sh relay (no account, no server of our own): each launcher
//! posts its nickname and, when hosting, the full address of its game
//! server. Friends see it, copy it or join with one click.
//!
//! Budget: ntfy.sh allows ~250 posts/day per IP, so we post on join, on
//! change, on leave and a heartbeat every 4 minutes. Updates arrive over
//! one long-lived streaming connection.

use crate::net;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RELAY: &str = "https://ntfy.sh";
const HEARTBEAT: Duration = Duration::from_secs(240);
/// A member silent for this long is shown as gone.
pub const STALE_SECS: u64 = 600;

/// What one launcher tells the room about itself.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Presence {
    #[serde(default)]
    pub v: u8,
    pub id: String,
    pub nick: String,
    /// Address inside the home network (same Wi-Fi / LAN).
    #[serde(default)]
    pub lan: String,
    /// Internet address as seen from outside.
    #[serde(default)]
    pub public: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub host: bool,
    /// «Minecraft 1.20.1 · Forge» — which build the game server runs.
    #[serde(default)]
    pub mc: String,
    #[serde(default)]
    pub leave: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Checking,
    Open,
    Closed,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub presence: Presence,
    /// Unix time of the member's last message.
    pub seen: u64,
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn md5_hex(text: &str) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Fresh 5-digit code (10000–99999).
pub fn new_code() -> String {
    let seed = format!(
        "{:?}-{}-{:?}",
        SystemTime::now(),
        std::process::id(),
        Instant::now()
    );
    let hex = md5_hex(&seed);
    let n = u64::from_str_radix(&hex[..12], 16).unwrap_or(0);
    format!("{}", 10_000 + n % 90_000)
}

/// Accepts «12345», « 12 345 » and the like; anything else is rejected.
pub fn parse_code(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    (digits.len() == 5 && digits.chars().all(|c| c.is_ascii_digit())).then_some(digits)
}

/// Relay topic for a code: hashed, so the code itself never appears in URLs.
pub fn topic(code: &str) -> String {
    format!("llmmadrid-{}", &md5_hex(&format!("madrid-room-v1:{code}"))[..20])
}

/// Stable id for this install, kept in the config.
pub fn device_id(existing: &str) -> String {
    if existing.len() >= 8 {
        return existing.to_string();
    }
    md5_hex(&format!("{:?}-{}", SystemTime::now(), std::process::id()))[..16].to_string()
}

/// True when both launchers share the same internet IP and the host has a LAN address.
pub fn same_network(my_public: &str, host: &Presence) -> bool {
    !host.lan.is_empty() && !my_public.is_empty() && host.public == my_public
}

/// The address a friend should use to reach `host`: the LAN one when both
/// sit behind the same internet address, the public one otherwise.
pub fn best_address(my_public: &str, host: &Presence) -> String {
    let ip = if !host.lan.is_empty() && (host.public.is_empty() || host.public == my_public) {
        &host.lan
    } else if !host.public.is_empty() {
        &host.public
    } else {
        &host.lan
    };
    if ip.is_empty() {
        String::new()
    } else {
        format!("{ip}:{}", host.port)
    }
}

/// Applies one relay message to the member table.
pub fn apply(members: &mut HashMap<String, Member>, p: Presence, at: u64) {
    if p.id.is_empty() {
        return;
    }
    if p.leave {
        members.remove(&p.id);
        return;
    }
    let newer = members.get(&p.id).map(|m| m.seen <= at).unwrap_or(true);
    if newer {
        members.insert(p.id.clone(), Member { presence: p, seen: at });
    }
}

/// A joined room. Dropping it does not leave — call `leave`.
pub struct Room {
    pub code: String,
    topic: String,
    members: Arc<Mutex<HashMap<String, Member>>>,
    reach: Arc<Mutex<HashMap<String, (Instant, Reach)>>>,
    status: Arc<Mutex<String>>,
    me: Arc<Mutex<Presence>>,
    dirty: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

/// Streaming needs a longer read timeout than the shared client (the relay
/// sends a keepalive every ~45 s).
fn stream_client() -> reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!("MinecraftLauncherMadrid/{} (Windows)", crate::config::VERSION))
            .connect_timeout(Duration::from_secs(12))
            .read_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
    .clone()
}

async fn publish(topic: &str, p: &Presence) -> bool {
    let Ok(body) = serde_json::to_string(p) else {
        return false;
    };
    net::client()
        .post(format!("{RELAY}/{topic}"))
        .timeout(Duration::from_secs(12))
        .body(body)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

impl Room {
    pub fn join(code: &str, me: Presence, ctx: egui::Context) -> Room {
        let room = Room {
            code: code.to_string(),
            topic: topic(code),
            members: Arc::new(Mutex::new(HashMap::new())),
            reach: Arc::new(Mutex::new(HashMap::new())),
            status: Arc::new(Mutex::new("подключаюсь к комнате…".into())),
            me: Arc::new(Mutex::new(me)),
            dirty: Arc::new(AtomicBool::new(true)),
            stop: Arc::new(AtomicBool::new(false)),
        };
        room.spawn_listener(ctx.clone());
        room.spawn_heartbeat(ctx);
        room
    }

    fn spawn_listener(&self, ctx: egui::Context) {
        let topic = self.topic.clone();
        let members = self.members.clone();
        let status = self.status.clone();
        let stop = self.stop.clone();
        net::spawn(async move {
            use futures_util::StreamExt;
            #[derive(Deserialize)]
            struct Event {
                #[serde(default)]
                id: String,
                #[serde(default)]
                time: u64,
                #[serde(default)]
                event: String,
                #[serde(default)]
                message: String,
            }
            let mut since = "all".to_string();
            while !stop.load(Ordering::Relaxed) {
                let url = format!("{RELAY}/{topic}/json?since={since}");
                match stream_client().get(url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        set(&status, "в комнате");
                        ctx.request_repaint();
                        let mut buf: Vec<u8> = Vec::new();
                        let mut stream = resp.bytes_stream();
                        while let Some(chunk) = stream.next().await {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }
                            let Ok(chunk) = chunk else { break };
                            buf.extend_from_slice(&chunk);
                            while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                                let line: Vec<u8> = buf.drain(..=pos).collect();
                                let Ok(ev) = serde_json::from_slice::<Event>(&line) else { continue };
                                if ev.event != "message" {
                                    continue;
                                }
                                since = ev.id.clone();
                                // Old rooms with the same code are ignored.
                                if ev.time.saturating_add(STALE_SECS) < now_unix() {
                                    continue;
                                }
                                if let Ok(p) = serde_json::from_str::<Presence>(&ev.message) {
                                    if let Ok(mut m) = members.lock() {
                                        apply(&mut m, p, ev.time);
                                    }
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                    Ok(resp) => set(&status, &format!("комната недоступна ({}) — переподключаюсь", resp.status().as_u16())),
                    Err(_) => set(&status, "нет связи с комнатой — переподключаюсь"),
                }
                ctx.request_repaint();
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }

    fn spawn_heartbeat(&self, ctx: egui::Context) {
        let topic = self.topic.clone();
        let me = self.me.clone();
        let dirty = self.dirty.clone();
        let stop = self.stop.clone();
        let status = self.status.clone();
        net::spawn(async move {
            let mut last: Option<Instant> = None;
            while !stop.load(Ordering::Relaxed) {
                let due = last.map(|t| t.elapsed() >= HEARTBEAT).unwrap_or(true);
                if dirty.swap(false, Ordering::Relaxed) || due {
                    // Coalesce bursts of edits (typing a port) into one post.
                    tokio::time::sleep(Duration::from_millis(700)).await;
                    dirty.store(false, Ordering::Relaxed);
                    let p = me.lock().map(|p| p.clone()).unwrap_or_default();
                    if !publish(&topic, &p).await {
                        set(&status, "не удалось отправить в комнату — повторю");
                        dirty.store(true, Ordering::Relaxed);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                    last = Some(Instant::now());
                    ctx.request_repaint();
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }

    pub fn status(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn me(&self) -> Presence {
        self.me.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Changes what we announce; the change is posted right away.
    pub fn update_me(&self, f: impl FnOnce(&mut Presence)) {
        if let Ok(mut p) = self.me.lock() {
            let before = p.clone();
            f(&mut p);
            if *p != before {
                self.dirty.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Live members, hosts first, then by nickname. Includes us.
    pub fn members(&self) -> Vec<Member> {
        let now = now_unix();
        let me = self.me();
        let mut list: Vec<Member> = self
            .members
            .lock()
            .map(|m| m.values().filter(|m| m.seen.saturating_add(STALE_SECS) >= now).cloned().collect())
            .unwrap_or_default();
        if !list.iter().any(|m| m.presence.id == me.id) {
            list.push(Member { presence: me, seen: now });
        }
        list.sort_by(|a, b| {
            b.presence
                .host
                .cmp(&a.presence.host)
                .then(a.presence.nick.to_lowercase().cmp(&b.presence.nick.to_lowercase()))
        });
        list
    }

    /// Whether `addr` accepts TCP right now, re-checked every 20 s.
    pub fn reach(&self, member_id: &str, addr: &str, ctx: &egui::Context) -> Reach {
        let current = self.reach.lock().ok().and_then(|m| m.get(member_id).copied());
        if let Some((at, r)) = current {
            if r == Reach::Checking || at.elapsed() < Duration::from_secs(20) {
                return r;
            }
        }
        if let Ok(mut m) = self.reach.lock() {
            m.insert(member_id.to_string(), (Instant::now(), Reach::Checking));
        }
        let reach = self.reach.clone();
        let id = member_id.to_string();
        let addr = addr.to_string();
        let ctx = ctx.clone();
        net::spawn(async move {
            let ok = tokio::task::spawn_blocking(move || {
                use std::net::ToSocketAddrs;
                addr.to_socket_addrs()
                    .ok()
                    .and_then(|mut a| a.next())
                    .map(|sa| std::net::TcpStream::connect_timeout(&sa, Duration::from_secs(3)).is_ok())
                    .unwrap_or(false)
            })
            .await
            .unwrap_or(false);
            if let Ok(mut m) = reach.lock() {
                m.insert(id, (Instant::now(), if ok { Reach::Open } else { Reach::Closed }));
            }
            ctx.request_repaint();
        });
        current.map(|c| c.1).unwrap_or(Reach::Checking)
    }

    /// Tells the room we left, then stops. Blocks up to 3 s when called
    /// outside the runtime (on exit), so friends see us go immediately.
    pub fn leave(self, blocking: bool) {
        self.stop.store(true, Ordering::Relaxed);
        let mut p = self.me();
        p.leave = true;
        let topic = self.topic.clone();
        if blocking {
            let _ = net::rt().block_on(async {
                tokio::time::timeout(Duration::from_secs(3), publish(&topic, &p)).await
            });
        } else {
            net::spawn(async move {
                publish(&topic, &p).await;
            });
        }
    }
}

fn set(status: &Arc<Mutex<String>>, text: &str) {
    if let Ok(mut s) = status.lock() {
        *s = text.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, host: bool) -> Presence {
        Presence {
            v: 1,
            id: id.into(),
            nick: id.into(),
            lan: "192.168.1.5".into(),
            public: "93.184.216.34".into(),
            port: 25565,
            host,
            ..Default::default()
        }
    }

    #[test]
    fn codes_are_five_digits_and_parse_back() {
        for _ in 0..20 {
            let c = new_code();
            assert_eq!(c.len(), 5);
            assert_eq!(parse_code(&c).as_deref(), Some(c.as_str()));
        }
        assert_eq!(parse_code(" 12 345 ").as_deref(), Some("12345"));
        assert_eq!(parse_code("1234"), None);
        assert_eq!(parse_code("12a45"), None);
    }

    #[test]
    fn topic_hides_the_code_and_is_stable() {
        assert_eq!(topic("12345"), topic("12345"));
        assert_ne!(topic("12345"), topic("12346"));
        assert!(!topic("12345").contains("12345"));
        assert!(topic("12345").starts_with("llmmadrid-"));
    }

    #[test]
    fn same_network_friends_use_lan_address() {
        let host = p("h", true);
        assert!(same_network("93.184.216.34", &host));
        assert!(!same_network("1.1.1.1", &host));
        assert_eq!(best_address("93.184.216.34", &host), "192.168.1.5:25565");
        assert_eq!(best_address("1.1.1.1", &host), "93.184.216.34:25565");
        let empty = Presence { port: 25565, host: true, ..Default::default() };
        assert_eq!(best_address("1.1.1.1", &empty), "");
    }

    /// Live check against the real relay: two launchers with one code see
    /// each other, and a leave removes the member. `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn two_launchers_meet_through_the_relay() {
        let code = new_code();
        let ctx = egui::Context::default();
        let a = Room::join(&code, p("alice-test", true), ctx.clone());
        let b = Room::join(&code, p("bob-test", false), ctx);
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut seen = false;
        while Instant::now() < deadline {
            let ids: Vec<String> = b.members().into_iter().map(|m| m.presence.id).collect();
            if ids.contains(&"alice-test".to_string()) && a.members().iter().any(|m| m.presence.id == "bob-test") {
                seen = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        assert!(seen, "members did not meet; status a={} b={}", a.status(), b.status());
        let host = b.members().into_iter().find(|m| m.presence.id == "alice-test").unwrap();
        assert!(host.presence.host);
        assert_eq!(host.presence.port, 25565);
        a.leave(true);
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut gone = false;
        while Instant::now() < deadline {
            if !b.members().iter().any(|m| m.presence.id == "alice-test") {
                gone = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        assert!(gone, "leave was not seen");
        b.leave(true);
    }

    #[test]
    fn leave_removes_and_old_messages_do_not_override() {
        let mut m = HashMap::new();
        apply(&mut m, p("a", false), 100);
        apply(&mut m, p("a", true), 200);
        apply(&mut m, p("a", false), 150);
        assert!(m["a"].presence.host, "older message must not win");
        let mut gone = p("a", false);
        gone.leave = true;
        apply(&mut m, gone, 300);
        assert!(m.is_empty());
    }
}
