use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
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
    pub light: f32,
    pub odor: f32,
    pub touch: f32,
    pub temperature: f32,
    pub drive: String,
    pub decision: String,
    pub attention: String,
    pub confidence: f32,
    pub puff_level: f32,
    pub puff_count: u64,
    pub puff_enabled: bool,
    pub reward: f32,
    pub novelty: f32,
    pub strategy: String,
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
    pub total_puff_events: u64,
    pub learning_updates: u64,
    pub active_flies: usize,
    pub sim_time: f32,
    pub fixed_timestep_ms: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdventureSnapshot {
    pub id: String,
    pub name: String,
    pub objective: String,
    pub progress: f32,
    pub score: u32,
    pub target: [f32; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorSnapshot {
    pub tick: u64,
    pub running: bool,
    pub speed: f32,
    pub decay: f32,
    pub selected_fly: Option<u32>,
    pub agents: Vec<AgentSnapshot>,
    pub adventure: AdventureSnapshot,
    pub metrics: EditorMetrics,
    pub model_note: String,
}

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    id: Option<u32>,
    value: Option<f32>,
    name: Option<String>,
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
    light: f32,
    odor: f32,
    touch: f32,
    temperature: f32,
    puff_level: f32,
    puff_count: u64,
    puff_enabled: bool,
    puff_just_started: bool,
    reward: f32,
    novelty: f32,
    strategy: &'static str,
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
            light: 0.5,
            odor: 0.5,
            touch: 0.0,
            temperature: 0.5,
            puff_level: 0.0,
            puff_count: 0,
            puff_enabled: true,
            puff_just_started: false,
            reward: 0.0,
            novelty: 0.5,
            strategy: "balanced",
            hormone_level: 0.0,
            hormone_enabled: index.is_multiple_of(2),
            channel: if index == 0 { "input" } else { "internal" },
        }
    }

    fn step(&mut self, time: f32, dt: f32, speed: f32, decay: f32) {
        let phase = time * 0.9 + self.id as f32 * 1.31;
        self.light = (0.5 + 0.35 * (time * 0.7).sin()).clamp(0.0, 1.0);
        self.odor = (0.5 + 0.3 * (time * 0.43 + self.id as f32 * 0.17).sin()).clamp(0.0, 1.0);
        self.temperature = (0.5 + 0.15 * (time * 0.2).sin()).clamp(0.0, 1.0);
        let boundary = ((self.position[0].abs() - 4.5).max(0.0)
            + (self.position[1].abs() - 2.8).max(0.0))
        .clamp(0.0, 1.0);
        self.touch = ((self.velocity[0].abs() + self.velocity[1].abs()) * 0.2 + boundary * 0.8)
            .clamp(0.0, 1.0);
        self.input_level =
            (self.light * 0.35 + self.odor * 0.45 + self.touch * 0.2).clamp(0.0, 1.0);
        let turn = ((self.odor - 0.5) * 1.4 + (self.light - 0.5) * 0.3 - self.touch * 0.8)
            .clamp(-1.0, 1.0);
        let throttle = (0.22 + self.light * 0.3 + self.energy * 0.2).clamp(0.0, 1.0);
        let wave = (0.5 + 0.5 * (phase * 2.0).sin()) * 0.5 + 0.5;
        let reaction = (turn * 0.5 + wave * 0.5).abs() * decay;
        self.reaction_level = reaction.clamp(0.0, 1.0);

        let desired_x = (self.odor - 0.5) * 2.4 + (time * 0.35 + self.id as f32).sin() * 0.9
            - self.position[0] * 0.08;
        let desired_y = (self.light - 0.5) * 1.6 + (time * 0.27 + self.id as f32 * 1.7).cos() * 0.7
            - self.position[1] * 0.1;
        let desired_z = 1.25 + (time * 0.8 + self.id as f32).sin() * 0.45;
        let desired = [desired_x, desired_y, desired_z];
        for (axis, target) in desired.iter().enumerate() {
            self.velocity[axis] += (*target - self.velocity[axis]) * dt * 2.4 * speed;
            self.position[axis] += self.velocity[axis] * dt * speed;
        }
        for (axis, limit) in [(0, 5.5), (1, 3.5)] {
            if self.position[axis].abs() > limit {
                self.position[axis] = self.position[axis].clamp(-limit, limit);
                self.velocity[axis] *= -0.45;
            }
        }
        self.position[2] = self.position[2].clamp(0.6, 3.0);
        self.energy = (self.energy - dt * speed * (0.001 + throttle * 0.0015)).clamp(0.0, 1.0);
        self.stress = (self.stress * 0.985 + self.touch * dt * 0.015).clamp(0.0, 1.0);
        if self.hormone_enabled {
            let target = ((self.odor - 0.55).max(0.0) * 0.8 + self.stress * 0.4).clamp(0.0, 1.0);
            self.hormone_level = (self.hormone_level * 0.93 + target * 0.07).clamp(0.0, 1.0);
        } else {
            self.hormone_level = (self.hormone_level * 0.9).clamp(0.0, 1.0);
        }
        self.puff_just_started = false;
        let cycle = (time * speed * 0.55 + self.id as f32 * 1.9).rem_euclid(6.0);
        let nervous = (phase.sin() * 0.5 + 0.5) * self.stress;
        if self.puff_enabled && (cycle < dt * 1.5 || nervous > 0.85) && self.puff_level < 0.25 {
            self.puff_count = self.puff_count.saturating_add(1);
            self.puff_level = 1.0;
            self.puff_just_started = true;
            self.stress = (self.stress + 0.04).min(1.0);
        }
        self.puff_level = (self.puff_level - dt * 0.5).clamp(0.0, 1.0);
        self.novelty = (self.novelty * 0.97 + (1.0 - self.reward) * dt * 0.02).clamp(0.0, 1.0);
        self.strategy = if self.reward > 0.6 {
            "exploit_reward"
        } else if self.novelty > 0.68 {
            "explore_novelty"
        } else {
            "balanced"
        };
    }

    fn intent(&self) -> (&'static str, &'static str, f32) {
        let drive = if self.touch > 0.35 {
            "avoid_contact"
        } else if self.odor > 0.62 {
            "approach_odor"
        } else if self.light > 0.72 {
            "seek_light"
        } else {
            "explore"
        };
        let decision = if self.touch > 0.35 {
            "break_contact"
        } else if self.odor > 0.62 {
            "steer_toward_odor"
        } else if self.position[2] < 0.85 {
            "climb"
        } else {
            "maintain_course"
        };
        let confidence = (0.45 + self.input_level * 0.45 + self.energy * 0.1).clamp(0.0, 1.0);
        (drive, decision, confidence)
    }
}

fn adventure_spec(id: &str) -> (&'static str, &'static str) {
    match id {
        "odor_trail" => ("odor_trail", "Следуй за подвижным запахом"),
        "ring_circuit" => ("ring_circuit", "Пролети кольцо арены три раза"),
        "hormone_calibration" => ("hormone_calibration", "Собери сигнал и выпусти гормон"),
        _ => ("free_flight", "Свободный полёт и исследование"),
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
    total_puffs: u64,
    learning_updates: u64,
    adventure_id: String,
    adventure_score: u32,
    adventure_target: [f32; 2],
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
            total_puffs: 0,
            learning_updates: 0,
            adventure_id: "free_flight".to_owned(),
            adventure_score: 0,
            adventure_target: [0.0, 0.0],
        }
    }

    fn update_adventure_target(&mut self) {
        self.adventure_target = match self.adventure_id.as_str() {
            "odor_trail" => [
                (self.time * 0.8).sin() * 4.0,
                (self.time * 0.55).cos() * 2.2,
            ],
            "ring_circuit" => [(self.time * 1.2).cos() * 3.8, (self.time * 1.2).sin() * 2.4],
            "hormone_calibration" => [(self.time * 0.4).sin() * 3.0, (self.time * 0.6).cos() * 2.0],
            _ => [0.0, 0.0],
        };
    }

    pub fn step(&mut self, dt: f32) {
        if !self.running {
            return;
        }
        self.time += dt * self.speed;
        self.tick += 1;
        self.update_adventure_target();
        let target = self.adventure_target;
        for agent in &mut self.agents {
            agent.step(self.time, dt, self.speed, self.decay);
            self.total_reactions += 1;
            if agent.hormone_level > 0.25 {
                self.total_hormones += 1;
            }
            if agent.puff_just_started {
                self.total_puffs = self.total_puffs.saturating_add(1);
            }
            if self.adventure_id != "free_flight" {
                let dx = agent.position[0] - target[0];
                let dy = agent.position[1] - target[1];
                if (dx * dx + dy * dy).sqrt() < 0.85 {
                    self.adventure_score = self.adventure_score.saturating_add(1);
                    self.learning_updates += 1;
                    agent.reward = (agent.reward + 0.08).min(1.0);
                }
            }
        }
    }

    pub fn snapshot(&self, fps: f32) -> EditorSnapshot {
        let agents = self
            .agents
            .iter()
            .map(|agent| {
                let (drive, decision, confidence) = agent.intent();
                AgentSnapshot {
                    id: agent.id,
                    name: agent.name.clone(),
                    position: agent.position,
                    velocity: agent.velocity,
                    energy: agent.energy,
                    stress: agent.stress,
                    neurotransmitter: agent.neurotransmitter.to_owned(),
                    input_level: agent.input_level,
                    reaction_level: agent.reaction_level,
                    light: agent.light,
                    odor: agent.odor,
                    touch: agent.touch,
                    temperature: agent.temperature,
                    drive: drive.to_owned(),
                    decision: decision.to_owned(),
                    attention: format!(
                        "odor {:.0}% · light {:.0}% · touch {:.0}%",
                        agent.odor * 100.0,
                        agent.light * 100.0,
                        agent.touch * 100.0
                    ),
                    confidence,
                    puff_level: agent.puff_level,
                    puff_count: agent.puff_count,
                    puff_enabled: agent.puff_enabled,
                    reward: agent.reward,
                    novelty: agent.novelty,
                    strategy: agent.strategy.to_owned(),
                    hormone_level: agent.hormone_level,
                    hormone_enabled: agent.hormone_enabled,
                    channel: agent.channel.to_owned(),
                    selected: self.selected == Some(agent.id),
                }
            })
            .collect();
        EditorSnapshot {
            tick: self.tick,
            running: self.running,
            speed: self.speed,
            decay: self.decay,
            selected_fly: self.selected,
            agents,
            adventure: {
                let (name, objective) = adventure_spec(&self.adventure_id);
                AdventureSnapshot {
                    id: self.adventure_id.clone(),
                    name: name.to_owned(),
                    objective: objective.to_owned(),
                    progress: if self.adventure_id == "free_flight" { 1.0 } else { (self.adventure_score % 100) as f32 / 100.0 },
                    score: self.adventure_score,
                    target: self.adventure_target,
                }
            },
            metrics: EditorMetrics {
                fps,
                total_reactions: self.total_reactions,
                total_hormone_pulses: self.total_hormones,
                total_puff_events: self.total_puffs,
                learning_updates: self.learning_updates,
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
            "puff" => {
                let id = command.id.context("puff requires id")?;
                let mut changed = false;
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.puff_enabled = true;
                    agent.puff_level = 1.0;
                    agent.puff_count = agent.puff_count.saturating_add(1);
                    changed = true;
                }
                if changed {
                    self.total_puffs = self.total_puffs.saturating_add(1);
                }
            }
            "reward" => {
                let id = command.id.context("reward requires id")?;
                let mut changed = false;
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.reward = (agent.reward + 0.2).min(1.0);
                    changed = true;
                }
                if changed {
                    self.learning_updates = self.learning_updates.saturating_add(1);
                }
            }
            "adventure" => {
                let requested = command.name.context("adventure requires name")?;
                let (id, _) = adventure_spec(&requested);
                self.adventure_id = id.to_owned();
                self.adventure_score = 0;
                self.update_adventure_target();
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
                    let id = self.agents.iter().map(|agent| agent.id).max().unwrap_or(0) + 1;
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

fn serve_client(stream: TcpStream, runtime: &Arc<Mutex<EditorRuntime>>, fps: f32) {
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
            let snapshot = runtime
                .lock()
                .expect("editor runtime lock poisoned")
                .snapshot(fps);
            match serde_json::to_string(&snapshot) {
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
                flies: runtime
                    .lock()
                    .expect("editor runtime lock poisoned")
                    .snapshot(fps)
                    .agents
                    .len(),
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
            match runtime
                .lock()
                .expect("editor runtime lock poisoned")
                .apply_command(&body)
            {
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
    let runtime = Arc::new(Mutex::new(EditorRuntime::new(flies)));
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
            runtime
                .lock()
                .expect("editor runtime lock poisoned")
                .step(FIXED_DT);
            accumulator -= FIXED_DT;
        }
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let client_runtime = Arc::clone(&runtime);
                    let client_fps = fps;
                    thread::spawn(move || serve_client(stream, &client_runtime, client_fps));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_flies_keep_moving_and_expose_intent() {
        let mut runtime = EditorRuntime::new(3);
        for _ in 0..600 {
            runtime.step(FIXED_DT);
        }
        let snapshot = runtime.snapshot(60.0);
        assert_eq!(snapshot.agents.len(), 3);
        assert!(
            snapshot
                .agents
                .iter()
                .all(|agent| agent.position.iter().all(|value| value.is_finite()))
        );
        assert!(
            snapshot
                .agents
                .iter()
                .all(|agent| !agent.drive.is_empty() && !agent.decision.is_empty())
        );
        assert!(snapshot.metrics.total_reactions > 0);
    }

    #[test]
    fn editor_switches_and_toggles_selected_agent() {
        let mut runtime = EditorRuntime::new(3);
        runtime
            .apply_command(br#"{"action":"select","id":2}"#)
            .expect("select");
        runtime
            .apply_command(br#"{"action":"hormone","id":2,"enabled":false}"#)
            .expect("hormone");
        let snapshot = runtime.snapshot(60.0);
        assert_eq!(snapshot.selected_fly, Some(2));
        assert!(
            !snapshot
                .agents
                .iter()
                .find(|agent| agent.id == 2)
                .expect("agent")
                .hormone_enabled
        );
    }

    #[test]
    fn editor_adventure_puff_and_reward_commands() {
        let mut runtime = EditorRuntime::new(2);
        runtime
            .apply_command(br#"{"action":"adventure","name":"ring_circuit"}"#)
            .expect("adventure");
        runtime
            .apply_command(br#"{"action":"puff","id":1}"#)
            .expect("puff");
        runtime
            .apply_command(br#"{"action":"reward","id":1}"#)
            .expect("reward");
        let snapshot = runtime.snapshot(60.0);
        assert_eq!(snapshot.adventure.id, "ring_circuit");
        assert!(!snapshot.adventure.objective.is_empty());
        let agent = snapshot
            .agents
            .iter()
            .find(|agent| agent.id == 1)
            .expect("agent");
        assert_eq!(agent.puff_count, 1);
        assert!(agent.puff_level > 0.5);
        assert!(agent.reward > 0.0);
        assert_eq!(snapshot.metrics.total_puff_events, 1);
        assert_eq!(snapshot.metrics.learning_updates, 1);
    }

    #[test]
    fn editor_adventure_target_follows_time() {
        let mut runtime = EditorRuntime::new(2);
        runtime
            .apply_command(br#"{"action":"adventure","name":"odor_trail"}"#)
            .expect("adventure");
        let first = runtime.snapshot(60.0).adventure.target;
        for _ in 0..120 {
            runtime.step(FIXED_DT);
        }
        let second = runtime.snapshot(60.0).adventure.target;
        assert!((first[0] - second[0]).abs() > 0.001 || (first[1] - second[1]).abs() > 0.001);
    }
}
