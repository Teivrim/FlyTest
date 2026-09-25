use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 8765;
pub const DEFAULT_FLIES: usize = 3;
const MAX_FLIES: usize = 8;
const SERVER_FPS: f32 = 60.0;
const FIXED_DT: f32 = 1.0 / SERVER_FPS;

const INDEX_HTML: &str = include_str!("../editor/index.html");
const THREE_JS: &str = include_str!("../editor/three.min.js");
const APP_JS: &str = include_str!("../editor/app.js");
const STYLE_CSS: &str = include_str!("../editor/style.css");

#[derive(Debug, Clone, Serialize)]
pub struct EditorConfig {
    pub port: u16,
    pub flies: usize,
    pub max_flies: usize,
    pub server_fps: u32,
    pub model: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSnapshot {
    pub id: u32,
    pub name: String,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub energy: f32,
    pub stress: f32,
    pub neurotransmitter: String,
    pub input_level: f32,
    pub reaction_level: f32,
    pub hormone_level: f32,
    pub hormone_enabled: bool,
    pub channel: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorMetrics {
    pub fps: f32,
    pub total_reactions: u64,
    pub total_hormone_pulses: u64,
    pub active_flies: usize,
    pub sim_time: f32,
    pub fixed_timestep_ms: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorSnapshot {
    pub tick: u64,
    pub running: bool,
    pub speed: f32,
    pub decay: f32,
    pub selected_fly: Option<u32>,
    pub agents: Vec<AgentSnapshot>,
    pub metrics: EditorMetrics,
    pub model_note: String,
}

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    id: Option<u32>,
    value: Option<f32>,
    enabled: Option<bool>,
}

#[derive(Debug, Clone)]
struct Agent {
    id: u32,
    name: String,
    position: [f32; 3],
    velocity: [f32; 3],
    energy: f32,
    stress: f32,
    neurotransmitter: &'static str,
    input_level: f32,
    reaction_level: f32,
    hormone_level: f32,
    hormone_enabled: bool,
    channel: &'static str,
}

impl Agent {
    fn new(id: u32, index: usize) -> Self {
        let angle = index as f32 * 1.7;
        let names = [
            "Scout",
            "Forager",
            "Navigator",
            "Dancer",
            "Pilot",
            "Watcher",
            "Courier",
            "Moth",
        ];
        Self {
            id,
            name: names[index % names.len()].to_owned(),
            position: [angle.cos() * 2.0, angle.sin() * 2.0, 1.0],
            velocity: [0.0, 0.0, 0.0],
            energy: 1.0,
            stress: 0.0,
            neurotransmitter: ["ACH", "GABA", "GLUT"][index % 3],
            input_level: 0.5,
            reaction_level: 0.5,
            hormone_level: 0.0,
            hormone_enabled: index.is_multiple_of(2),
            channel: if index == 0 { "input" } else { "internal" },
        }
    }

    fn step(&mut self, time: f32, dt: f32, speed: f32, decay: f32) {
        let phase = time * 0.9 + self.id as f32 * 1.31;
        let light = (0.5 + 0.35 * (time * 0.7).sin()).clamp(0.0, 1.0);
        let odor = (0.5 + 0.3 * (time * 0.43 + self.id as f32 * 0.17).sin()).clamp(0.0, 1.0);
        let touch = (self.velocity[0].abs() + self.velocity[1].abs()).clamp(0.0, 1.0);
        let temperature = (0.5 + 0.15 * (time * 0.2).sin()).clamp(0.0, 1.0);
        self.input_level = (light * 0.35 + odor * 0.45 + touch * 0.2).clamp(0.0, 1.0);
        let turn = ((odor - 0.5) * 1.4 + (light - 0.5) * 0.3 - touch * 0.55).clamp(-1.0, 1.0);
        let throttle = (0.22 + light * 0.3 + self.energy * 0.2).clamp(0.0, 1.0);
        let vertical = ((temperature - 0.5) * 0.4).clamp(-1.0, 1.0);
        let wave = (0.5 + 0.5 * (phase * 2.0).sin()) * 0.5 + 0.5;
        let reaction = (turn * 0.5 + wave * 0.5).abs() * decay;
        self.reaction_level = reaction.clamp(0.0, 1.0);

        self.velocity[0] += turn * dt * 1.6 * speed;
        self.velocity[1] += throttle * dt * 0.75 * speed;
        self.velocity[2] += vertical * dt * 0.35 * speed;
        for axis in 0..3 {
            self.velocity[axis] *= 0.965;
            self.position[axis] += self.velocity[axis] * dt * speed;
        }
        self.position[0] = self.position[0].clamp(-5.5, 5.5);
        self.position[1] = self.position[1].clamp(-3.5, 3.5);
        self.position[2] = (self.position[2] + self.velocity[2] * dt).clamp(0.6, 3.0);
        self.energy = (self.energy - dt * speed * (0.001 + throttle * 0.0015)).clamp(0.0, 1.0);
        self.stress = (self.stress * 0.985 + touch * dt * 0.015).clamp(0.0, 1.0);
        if self.hormone_enabled {
            let target = ((odor - 0.55).max(0.0) * 0.8 + self.stress * 0.4).clamp(0.0, 1.0);
            self.hormone_level = (self.hormone_level * 0.93 + target * 0.07).clamp(0.0, 1.0);
        } else {
            self.hormone_level = (self.hormone_level * 0.9).clamp(0.0, 1.0);
        }
    }
}

pub struct EditorRuntime {
    agents: Vec<Agent>,
    selected: Option<u32>,
    running: bool,
    speed: f32,
    decay: f32,
    time: f32,
    tick: u64,
    total_reactions: u64,
    total_hormones: u64,
}

impl EditorRuntime {
    pub fn new(count: usize) -> Self {
        let count = count.clamp(1, MAX_FLIES);
        let mut agents = Vec::with_capacity(count);
        for index in 0..count {
            agents.push(Agent::new(index as u32 + 1, index));
        }
        Self {
            selected: agents.first().map(|agent| agent.id),
            agents,
            running: true,
            speed: 1.0,
            decay: 0.85,
            time: 0.0,
            tick: 0,
            total_reactions: 0,
            total_hormones: 0,
        }
    }

    pub fn step(&mut self, dt: f32) {
        if !self.running {
            return;
        }
        self.time += dt * self.speed;
        self.tick += 1;
        for agent in &mut self.agents {
            agent.step(self.time, dt, self.speed, self.decay);
            self.total_reactions += 1;
            if agent.hormone_level > 0.25 {
                self.total_hormones += 1;
            }
        }
    }

    pub fn snapshot(&self, fps: f32) -> EditorSnapshot {
        let agents = self
            .agents
            .iter()
            .map(|agent| AgentSnapshot {
                id: agent.id,
                name: agent.name.clone(),
                position: agent.position,
                velocity: agent.velocity,
                energy: agent.energy,
                stress: agent.stress,
                neurotransmitter: agent.neurotransmitter.to_owned(),
                input_level: agent.input_level,
                reaction_level: agent.reaction_level,
                hormone_level: agent.hormone_level,
                hormone_enabled: agent.hormone_enabled,
                channel: agent.channel.to_owned(),
                selected: self.selected == Some(agent.id),
            })
            .collect();
        EditorSnapshot {
            tick: self.tick,
            running: self.running,
            speed: self.speed,
            decay: self.decay,
            selected_fly: self.selected,
            agents,
            metrics: EditorMetrics {
                fps,
                total_reactions: self.total_reactions,
                total_hormone_pulses: self.total_hormones,
                active_flies: self.agents.len(),
                sim_time: self.time,
                fixed_timestep_ms: FIXED_DT * 1000.0,
            },
            model_note: "Real-time editor uses a lightweight deterministic policy; swap in a model adapter for biological experiments.".to_owned(),
        }
    }

    pub fn apply_command(&mut self, body: &[u8]) -> Result<String> {
        let command: Command =
            serde_json::from_slice(body).context("invalid editor command JSON")?;
        match command.action.as_str() {
            "pause" => self.running = false,
            "resume" => self.running = true,
            "reset" => {
                let count = self.agents.len();
                *self = Self::new(count);
            }
            "select" => {
                let id = command.id.context("select requires id")?;
                if self.agents.iter().any(|agent| agent.id == id) {
                    self.selected = Some(id);
                }
            }
            "speed" => {
                self.speed = command.value.unwrap_or(self.speed).clamp(0.1, 3.0);
            }
            "decay" => {
                self.decay = command.value.unwrap_or(self.decay).clamp(0.0, 1.0);
            }
            "hormone" => {
                let id = command.id.context("hormone requires id")?;
                let enabled = command.enabled.unwrap_or(true);
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.hormone_enabled = enabled;
                }
            }
            "add" => {
                if self.agents.len() < MAX_FLIES {
                    let id = self.agents.len() as u32 + 1;
                    self.agents.push(Agent::new(id, self.agents.len()));
                    self.selected = Some(id);
                }
            }
            "remove" => {
                let id = command.id.context("remove requires id")?;
                if self.agents.len() > 1 {
                    self.agents.retain(|agent| agent.id != id);
                    self.selected = self.agents.first().map(|agent| agent.id);
                }
            }
            other => anyhow::bail!("unknown editor action: {other}"),
        }
        Ok(command.action)
    }
}

fn response(status: &str, content_type: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn write_response(stream: &mut TcpStream, bytes: &[u8]) {
    let _ = stream.write_all(bytes);
    let _ = stream.flush();
}

fn read_request<R: BufRead>(reader: &mut R) -> Result<Option<(String, String, Vec<u8>)>> {
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(None);
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path = parts.next().unwrap_or("/").to_owned();
    let mut content_length = 0_usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0_u8; content_length];
    if !body.is_empty() {
        reader.read_exact(&mut body)?;
    }
    Ok(Some((method, path, body)))
}

fn serve_client(stream: TcpStream, runtime: &mut EditorRuntime, fps: f32) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let Ok(read_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_stream);
    let mut writer = stream;
    loop {
        let request = match read_request(&mut reader) {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(_) => break,
        };
        let (method, path, body) = request;
        let reply = if method == "GET" && path == "/" {
            response("200 OK", "text/html; charset=utf-8", INDEX_HTML)
        } else if method == "GET" && path == "/three.min.js" {
            response("200 OK", "text/javascript; charset=utf-8", THREE_JS)
        } else if method == "GET" && path == "/app.js" {
            response("200 OK", "text/javascript; charset=utf-8", APP_JS)
        } else if method == "GET" && path == "/style.css" {
            response("200 OK", "text/css; charset=utf-8", STYLE_CSS)
        } else if method == "GET" && path == "/api/state" {
            match serde_json::to_string(&runtime.snapshot(fps)) {
                Ok(value) => response("200 OK", "application/json; charset=utf-8", &value),
                Err(error) => response(
                    "500 Internal Server Error",
                    "application/json",
                    &error.to_string(),
                ),
            }
        } else if method == "GET" && path == "/api/config" {
            let config = EditorConfig {
                port: DEFAULT_PORT,
                flies: runtime.snapshot(fps).agents.len(),
                max_flies: MAX_FLIES,
                server_fps: SERVER_FPS as u32,
                model: "lightweight-policy".to_owned(),
                note: "Use the JSONL/Blender bridge for full FlyTest scenes.".to_owned(),
            };
            match serde_json::to_string(&config) {
                Ok(value) => response("200 OK", "application/json; charset=utf-8", &value),
                Err(error) => response(
                    "500 Internal Server Error",
                    "application/json",
                    &error.to_string(),
                ),
            }
        } else if method == "POST" && path == "/api/command" {
            match runtime.apply_command(&body) {
                Ok(action) => response(
                    "200 OK",
                    "application/json",
                    &format!("{{\"ok\":true,\"action\":\"{action}\"}}"),
                ),
                Err(error) => response(
                    "400 Bad Request",
                    "application/json",
                    &format!("{{\"ok\":false,\"error\":\"{error}\"}}"),
                ),
            }
        } else {
            response("404 Not Found", "text/plain; charset=utf-8", "not found")
        };
        write_response(&mut writer, &reply);
    }
}

pub fn serve(port: u16, flies: usize) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("binding FlyEditor to http://127.0.0.1:{port}"))?;
    listener.set_nonblocking(true)?;
    let mut runtime = EditorRuntime::new(flies);
    println!("FlyEditor: http://127.0.0.1:{port}");
    println!("Press Ctrl+C to stop.");
    let mut previous = Instant::now();
    let mut accumulator = 0.0_f32;
    let mut fps = SERVER_FPS;
    loop {
        let now = Instant::now();
        let elapsed = now.duration_since(previous).as_secs_f32();
        previous = now;
        if elapsed > 0.0 {
            fps = fps * 0.9 + (1.0 / elapsed).min(240.0) * 0.1;
        }
        accumulator = (accumulator + elapsed).min(FIXED_DT * 4.0);
        while accumulator >= FIXED_DT {
            runtime.step(FIXED_DT);
            accumulator -= FIXED_DT;
        }
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    serve_client(stream, &mut runtime, fps);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
