use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::tfly::{self, action, cue};

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

/// The walk cycle of one character, for the renderer.
#[derive(Debug, Clone, Serialize)]
pub struct GaitSnapshot {
    /// Which preset the character walks with.
    pub preset: String,
    /// Accumulated walk phase in radians.
    pub phase: f32,
    /// Step length, 0..1.
    pub stride: f32,
    /// Step rate, 0..1.
    pub cadence: f32,
    /// How much the character wavers, 0..1.
    pub sway: f32,
    /// Stability, 1 upright.
    pub balance: f32,
    /// Lifetime step count.
    pub steps: u64,
    /// How strongly each gait has been learned, by preset name.
    pub weights: Vec<(String, f32)>,
    /// How well the character has learned to walk, 0..1.
    pub mastery: f32,
}

/// The face, straight out of the C core. The rig reads this and nothing else.
#[derive(Debug, Clone, Serialize)]
pub struct FaceSnapshot {
    /// 0 open, 1 shut.
    pub blink: f32,
    pub gaze_x: f32,
    pub gaze_y: f32,
    /// Dilation, 0.5 constricted to 1.6 wide.
    pub pupil: f32,
    /// -1 frowning to 1 raised.
    pub brow: f32,
    /// -1 frown to 1 smile.
    pub mouth: f32,
    pub mouth_open: f32,
    pub blush: f32,
    pub tears: f32,
    pub sweat: f32,
}

/// What a character is doing with her body, and who she is doing it to.
#[derive(Debug, Clone, Serialize)]
pub struct SocialSnapshot {
    /// Current pose, as a readable name.
    pub gesture: String,
    /// Envelope of the pose, 0..1.
    pub strength: f32,
    /// Progress through the pose, 0..1.
    pub phase: f32,
    /// Depth of the relationship, 0..1.
    pub bond: f32,
    /// The last encounter performed, readable.
    pub last_encounter: String,
    /// How inclined she is to start one, 0..1.
    pub drive: f32,
    /// Id of the fly she last met, if any.
    pub partner: Option<u32>,
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
    /// Body height above the floor, plus the walk cycle.
    pub height: f32,
    pub gait: GaitSnapshot,
    /// Every emotion, not just the dominant one, for the affect display.
    pub affect: Vec<(String, f32)>,
    /// The face, for the rig.
    pub face: FaceSnapshot,
    /// Gesture and relationship state.
    pub social: SocialSnapshot,
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

/// One notable thing that happened, kept so progress is auditable after the
/// fact rather than only observable in the instant.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    /// Simulated seconds since the runtime started.
    pub t: f32,
    /// Which fly it was about, or 0 for the whole troupe.
    pub fly: u32,
    /// Machine-readable kind, e.g. `level_up`, `mastered`, `pain`.
    pub kind: String,
    /// Human readable line.
    pub text: String,
}

/// A point on the selected fly's mastery curve.
#[derive(Debug, Clone, Serialize)]
pub struct CurvePoint {
    pub trial: u32,
    pub mastery: f32,
    pub weight: f32,
    pub gate: f32,
    pub accuracy: f32,
}

/// How much history to keep per fly and in the shared log.
const HISTORY_LEN: usize = 240;
const LOG_LEN: usize = 60;
/// Minimum sim time between two level-up lines for the same fly.
const LEVEL_LOG_INTERVAL: f32 = 3.0;
#[derive(Debug, Clone, Serialize)]
pub struct BrainSnapshot {
    pub act: String,
    pub mood: String,
    pub drive: String,
    pub thought: String,
    pub mastery: f32,
    pub trials: u32,
    pub correct: u32,
    pub xp: u32,
    pub level: u32,
    pub weight: f32,
    pub gate: f32,
    pub plasticity: f32,
    pub valence: f32,
    pub arousal: f32,
    pub pain: f32,
    pub energy: f32,
    pub stress: f32,
    pub memory: i32,
    pub last_lesson: String,
    pub last_outcome: String,
    /// Rolling mastery samples for the sparkline.
    pub curve: Vec<CurvePoint>,
}

/// One act in the act list, with the current fly's progress on it.
#[derive(Debug, Clone, Serialize)]
pub struct ActSnapshot {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub lesson: String,
    pub color: String,
    pub origin: [f32; 2],
    pub cue: String,
    pub solution: String,
    /// Mastery of the selected fly on this act, 0..1.
    pub mastery: f32,
    /// How strongly the fly currently associates the cue with the solution.
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrainingSnapshot {
    pub enabled: bool,
    pub epsilon: f32,
    pub interval: f32,
    pub current_act: usize,
    pub total_trials: u64,
    pub total_correct: u64,
    pub average_mastery: f32,
    /// Shared event log, newest last.
    pub log: Vec<LogEntry>,
    /// Where the JSONL mirror is being written, if anywhere.
    pub log_path: Option<String>,
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
    pub acts: Vec<ActSnapshot>,
    pub training: TrainingSnapshot,
    pub brain: Option<BrainSnapshot>,
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
    /// The stage this fly belongs to. Its policy walks it here.
    anchor: [f32; 2],
    /// Walk cycle state. These characters walk; they do not fly.
    gait_phase: f32,
    gait_stride: f32,
    gait_cadence: f32,
    gait_sway: f32,
    steps: u64,
    balance: f32,
    /// Which of the four gaits the character currently walks with.
    gait: usize,
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
            // These characters are on the ground, so the third component is
            // always zero. It used to be altitude.
            position: [angle.cos() * 2.0, angle.sin() * 2.0, 0.0],
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
            anchor: [0.0, 0.0],
            gait_phase: 0.0,
            gait_stride: 0.45,
            gait_cadence: 0.5,
            gait_sway: 0.4,
            steps: 0,
            balance: 1.0,
            gait: 1,
        }
    }

    /// Nudge the fly toward a point, without teleporting it.
    ///
    /// The act's stage is set as the fly's own attraction point rather than as
    /// an external push. That matters: `step` drives velocity toward the
    /// agent's `desired` vector every tick, so an external nudge would be
    /// washed out within a few frames and the fly would never actually arrive.
    fn steer_towards(&mut self, target: [f32; 2], _gain: f32, _dt: f32) {
        self.anchor = target;
    }

    /// Walk toward a point, used when a lonely character goes looking for
    /// company. This is a short-range nudge, not the act attractor: the act
    /// still decides where she belongs, and a friend only pulls her briefly.
    fn walk_toward(&mut self, x: f32, y: f32, dt: f32) {
        let dx = x - self.position[0];
        let dy = y - self.position[1];
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < 1e-3 {
            return;
        }
        // Ease off on approach, so they do not collide and bounce apart.
        let pull = distance.min(1.0) * dt * 1.4;
        self.velocity[0] += (dx / distance) * pull;
        self.velocity[1] += (dy / distance) * pull;
    }

    /// Advance the body forward along its current heading, for `n` steps.
    ///
    /// Used by gait trials, where the C brain decides the gait and moves but
    /// the visible body has to follow, or the distance measured would be zero
    /// while the character was plainly walking.
    fn advance(&mut self, steps: u32, dt: f32) {
        let speed = 0.6 + 0.5 * self.gait_cadence;
        let distance = speed * dt * steps as f32;
        let heading = if self.velocity[0].abs() + self.velocity[1].abs() > 1e-4 {
            (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt()
        } else {
            1.0
        };
        self.position[0] += (self.velocity[0] / heading) * distance;
        self.position[1] += (self.velocity[1] / heading) * distance;
        for (axis, limit) in [(0, 9.2), (1, 9.2)] {
            self.position[axis] = self.position[axis].clamp(-limit, limit);
        }
    }

    fn step(&mut self, time: f32, dt: f32, speed: f32, decay: f32) {
        let phase = time * 0.9 + self.id as f32 * 1.31;
        self.light = (0.5 + 0.35 * (time * 0.7).sin()).clamp(0.0, 1.0);
        self.odor = (0.5 + 0.3 * (time * 0.43 + self.id as f32 * 0.17).sin()).clamp(0.0, 1.0);
        self.temperature = (0.5 + 0.15 * (time * 0.2).sin()).clamp(0.0, 1.0);
        let boundary = ((self.position[0].abs() - 8.0).max(0.0)
            + (self.position[1].abs() - 8.0).max(0.0))
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

        // The character walks to its act's stage. The attractor is the anchor
        // plus a slow per-character orbit, so a troupe arriving at one act
        // spreads into a loose formation instead of stacking.
        let orbit_r = 0.55 + 0.18 * self.id as f32;
        let orbit_a = time * 0.5 + self.id as f32 * 2.1;
        let goal_x = self.anchor[0] + orbit_r * orbit_a.cos();
        let goal_y = self.anchor[1] + orbit_r * orbit_a.sin();
        let desired_x = (goal_x - self.position[0]) * 0.9
            + (self.odor - 0.5) * 0.35
            + (time * 0.35 + self.id as f32).sin() * 0.2;
        let desired_y = (goal_y - self.position[1]) * 0.9
            + (self.light - 0.5) * 0.25
            + (time * 0.27 + self.id as f32 * 1.7).cos() * 0.15;
        // Walking speed is capped far below the old flight speed, and is
        // scaled by the learned stride rather than by raw thrust.
        let pace = (0.35 + 0.55 * self.gait_cadence) * (0.4 + 0.6 * self.gait_stride);
        let cap = pace * 1.6;
        for (axis, target) in [desired_x, desired_y].into_iter().enumerate() {
            self.velocity[axis] += (target - self.velocity[axis]) * dt * 2.6 * speed;
            self.velocity[axis] = self.velocity[axis].clamp(-cap, cap);
            self.position[axis] += self.velocity[axis] * dt * speed;
        }
        self.velocity[2] = 0.0;

        // ---- the walk cycle ----
        // Phase advances with distance covered, not with time, so a character
        // standing still keeps its feet still instead of marching on the spot.
        let moved = ((self.velocity[0] * dt).powi(2) + (self.velocity[1] * dt).powi(2)).sqrt();
        let previous = self.gait_phase;
        self.gait_phase =
            (self.gait_phase + moved * 7.5 * self.gait_stride) % std::f32::consts::TAU;
        if (previous as f64 / std::f64::consts::PI).floor()
            != (self.gait_phase as f64 / std::f64::consts::PI).floor()
        {
            self.steps = self.steps.saturating_add(1);
        }
        // Sway at speed is what costs a character its footing.
        let planar_speed = (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt();
        let instability = self.gait_sway * planar_speed * 0.5;
        self.balance = (self.balance - instability * dt).clamp(0.0, 1.0);
        if self.balance < 0.25 {
            self.velocity[0] *= 0.5;
            self.velocity[1] *= 0.5;
            self.balance = (self.balance + 0.25 * dt).clamp(0.0, 1.0);
            self.stress = (self.stress + 0.1 * dt).clamp(0.0, 1.0);
        }

        // Bound the arena generously, since the act stages sit at +-6.4.
        for (axis, limit) in [(0, 9.2), (1, 9.2)] {
            if self.position[axis].abs() > limit {
                self.position[axis] = self.position[axis].clamp(-limit, limit);
                self.velocity[axis] *= -0.45;
            }
        }
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

// ===========================================================================
// The five acts
//
// Each act is a place, a stimulus, and one thing the fly has to learn. The
// lesson is a cue/action pair, so the fly's own associative weights decide
// whether it solves the act. Nothing is scripted as a success animation: the
// mastery bar reflects the real weight the C core accumulated.
// ===========================================================================

/// A circus act: where it is, what it looks like, and what it teaches.
pub struct Act {
    pub id: &'static str,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub lesson: &'static str,
    /// Centre of the act's stage, in arena coordinates.
    pub origin: [f32; 2],
    /// Accent colour, used by both the 3D scene and the CSS.
    pub color: &'static str,
    /// The conditioned stimulus the act presents.
    pub cue: i32,
    /// The action that solves it.
    pub solution: i32,
    /// A plausible wrong answer, so the fly must discriminate.
    pub distractor: i32,
    /// The action the fly performs when it is idle in this act.
    pub ambience: i32,
}

pub const CANDIDATES: [i32; 6] = [
    action::DANCE,
    action::EAT,
    action::REST,
    action::TURN_R,
    action::TURN_L,
    action::FORWARD,
];

/// The four gaits a character can learn to walk with, in C-core order.
pub const GAIT_NAMES: [&str; 4] = ["торопливый", "ровный", "длинный", "петляющий"];

/// Russian label for every emotion, matching `T_EMO_*` order.
pub const EMOTION_NAMES: [&str; 26] = [
    "боль",
    "страх",
    "радость",
    "грусть",
    "злость",
    "отвращение",
    "удивление",
    "любопытство",
    "влечение",
    "жажда цели",
    "довольство",
    "тревога",
    "растерянность",
    "гордость",
    "благодарность",
    "облегчение",
    "разочарование",
    "надежда",
    "ревность",
    "застенчивость",
    "привязанность",
    "скука",
    "восторг",
    "сочувствие",
    "доверие",
    "тоска",
];

pub const ACTS: [Act; 5] = [
    Act {
        id: "main_stage",
        name: "Главная арена",
        subtitle: "Прожекторы, аплодисменты, купол",
        lesson: "Яркий свет вызывает танец",
        origin: [0.0, 0.0],
        color: "#ff5c7a",
        cue: cue::LIGHT,
        solution: action::DANCE,
        distractor: action::REST,
        ambience: action::FLAP,
    },
    Act {
        id: "labyrinth",
        name: "Лабиринт",
        subtitle: "Стены, повороты, ориентиры",
        lesson: "Вибрация пола ведёт к цели",
        origin: [6.4, 0.0],
        color: "#54d7e8",
        cue: cue::VIBRATION,
        solution: action::TURN_R,
        distractor: action::TURN_L,
        ambience: action::FORWARD,
    },
    Act {
        id: "garden",
        name: "Чародейный сад",
        subtitle: "Цветы, фруктовый аромат, тепло",
        lesson: "Запах фрукта ведёт к еде",
        origin: [0.0, 6.0],
        color: "#8ce06a",
        cue: cue::ODOR_FRUIT,
        solution: action::EAT,
        distractor: action::DANCE,
        ambience: action::FLAP,
    },
    Act {
        id: "factory",
        name: "Фабрика чудес",
        subtitle: "Шестерни, металл, громкий стук",
        lesson: "Стук требует точного движения",
        origin: [-6.4, 0.0],
        color: "#ffb86b",
        cue: cue::SOUND,
        solution: action::FORWARD,
        distractor: action::REST,
        ambience: action::FLAP,
    },
    Act {
        id: "void",
        name: "Пустота",
        subtitle: "Ни света, ни звука, ни края",
        lesson: "В темноте нужно замереть",
        origin: [0.0, -6.0],
        color: "#a98bff",
        cue: cue::DARK,
        solution: action::REST,
        distractor: action::TURN_L,
        ambience: action::REST,
    },
];

fn act_by_id(id: &str) -> Option<&'static Act> {
    ACTS.iter().find(|act| act.id == id)
}

/// Russian label for a conditioned stimulus.
fn cue_name(cue_id: i32) -> &'static str {
    match cue_id {
        cue::LIGHT => "яркий свет",
        cue::DARK => "темнота",
        cue::VIBRATION => "вибрация пола",
        cue::SOUND => "стук",
        cue::ODOR_FRUIT => "запах фрукта",
        cue::ODOR_FLOWER => "запах цветка",
        cue::ODOR_FEMALE => "запах самки",
        cue::TOUCH => "прикосновение",
        cue::TEMPERATURE => "температура",
        cue::VISUAL => "зрительный образ",
        cue::GRAVITY => "гравитация",
        _ => "запах самца",
    }
}

/// Russian label for a motor primitive.
fn action_name(action_id: i32) -> &'static str {
    match action_id {
        action::REST => "замереть",
        action::FORWARD => "лететь вперёд",
        action::TURN_L => "повернуть налево",
        action::TURN_R => "повернуть направо",
        action::UP => "набрать высоту",
        action::DOWN => "снизиться",
        action::FLAP => "махать крыльями",
        action::DANCE => "танцевать",
        action::EAT => "есть",
        action::MATE => "ухаживать",
        _ => "действовать",
    }
}

/// Actions a fly may consider in any act.
///
/// The set is deliberately small and shared across all five acts, for two
/// reasons: a fly never has to discriminate between a dozen options, and the
/// same weight matrix has to carry every lesson, so one act's learning
/// competes with the others exactly as it would in a real brain.
///
/// Every act's `solution` and `distractor` must appear here. The tests assert
/// that, because a distractor outside the set would silently become dead
/// weight: the fly could never earn the partial credit it implies.
pub use CANDIDATES as CANDIDATE_ACTIONS;

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
    /// One trained brain per fly, indexed like `agents`.
    brains: Vec<Brain>,
    /// Which act each fly is currently in.
    acts: Vec<usize>,
    /// The act the runtime is broadcasting to.
    current_act: usize,
    /// Epsilon for exploration while training.
    epsilon: f32,
    /// How often, in sim seconds, one training trial runs per fly.
    trial_interval: f32,
    trial_timer: f32,
    /// Whether the runtime keeps training the flies.
    training: bool,
    /// Aggregated across every fly, for the headline numbers.
    total_trials: u64,
    total_correct: u64,
    /// Recent notable events, newest last.
    log: std::collections::VecDeque<LogEntry>,
    /// Optional JSONL sink, so a session can be analysed after the fact.
    log_file: Option<std::fs::File>,
    log_path: Option<std::path::PathBuf>,
    /// Who each fly last met, for the readout.
    partners: Vec<Option<u32>>,
    /// Cadence of the social layer, independent of the training timer.
    encounter_timer: f32,
}

/// Per-fly training bookkeeping, on top of the C brain's own weights.
struct Brain {
    fly: tfly::Fly,
    /// Smoothed success rate over recent trials.
    mastery: f32,
    trials: u32,
    correct: u32,
    /// Experience points, awarded for correct trials.
    xp: u32,
    level: u32,
    /// The last lesson the fly attempted.
    last_lesson: &'static str,
    last_outcome: &'static str,
    /// A short, readable trace of what the fly was weighing up.
    thought: String,
    /// Rolling mastery samples, so a curve can be drawn without keeping every
    /// trial forever.
    history: std::collections::VecDeque<CurvePoint>,
    /// Per-act flags, so a "mastered" event is logged once rather than on
    /// every trial after saturation.
    mastered: [bool; 5],
    /// The fly's id, needed to write log lines from inside the brain.
    id: u32,
    /// Sim time of the last level-up line, so a burst that crosses many levels
    /// at once does not flood the log.
    last_level_log: f32,
}

impl Brain {
    fn new(seed: u32, id: u32) -> Self {
        let mut fly = tfly::Fly::new();
        fly.seed(seed);
        Self {
            fly,
            mastery: 0.0,
            trials: 0,
            correct: 0,
            xp: 0,
            level: 1,
            last_lesson: "",
            last_outcome: "ожидание",
            thought: "муха ещё ничего не пробовала".to_owned(),
            history: std::collections::VecDeque::with_capacity(HISTORY_LEN),
            mastered: [false; 5],
            id,
            last_level_log: f32::NEG_INFINITY,
        }
    }

    /// Mastery on a specific act, derived from the brain's own weight rather
    /// than from the smoothed running average. This is what lets the act list
    /// show which lesson is actually stuck.
    ///
    /// The weight is clamped to -1..1 inside the C core, so a saturated weight
    /// of 1.0 means the lesson is fully learned. Rescaling from 0 rather than
    /// from the midpoint matters: an untouched fly must read as 0% mastery,
    /// not 50%.
    fn mastery_for(&self, act: &Act) -> f32 {
        self.fly.weight(act.cue, act.solution).clamp(0.0, 1.0)
    }

    /// Fold a trial result into the fly's statistics and rewrite its trace.
    ///
    /// Returns any notable events so the caller can put them in the shared
    /// log. The brain itself does not own the log, because the log is a
    /// property of the runtime, not of one fly.
    fn record(
        &mut self,
        act: &Act,
        act_index: usize,
        result: &tfly::TrialResult,
        now: f32,
    ) -> Vec<LogEntry> {
        let mut events = Vec::new();
        self.trials = self.trials.saturating_add(1);
        let hit = result.outcome == tfly::TrialOutcome::Correct;
        if hit {
            self.correct = self.correct.saturating_add(1);
            self.xp = self.xp.saturating_add(10);
        } else {
            self.xp = self.xp.saturating_add(2);
        }
        let new_level = 1 + self.xp / 100;
        // A training burst can cross a dozen levels inside a single frame.
        // Logging each one buries every other event, so level-ups are rate
        // limited to one line per interval. The level a burst actually reached
        // is reported by the caller on its training line instead.
        if new_level > self.level && now - self.last_level_log >= LEVEL_LOG_INTERVAL {
            self.last_level_log = now;
            events.push(LogEntry {
                t: 0.0,
                fly: self.id,
                kind: "level_up".to_owned(),
                text: format!("уровень {new_level}"),
            });
        }
        self.level = new_level;
        // Smoothed, so a single bad trial does not erase progress.
        self.mastery = self.mastery * 0.9 + f32::from(hit) * 0.1;
        self.last_lesson = act.id;
        self.last_outcome = match result.outcome {
            tfly::TrialOutcome::Correct => "верно",
            tfly::TrialOutcome::Partial => "почти",
            tfly::TrialOutcome::Wrong => "ошибка",
        };

        // Log the moment a lesson is actually mastered, once.
        let weight = self.fly.weight(act.cue, act.solution);
        if act_index < self.mastered.len() {
            if !self.mastered[act_index] && weight >= 0.9 {
                self.mastered[act_index] = true;
                events.push(LogEntry {
                    t: 0.0,
                    fly: self.id,
                    kind: "mastered".to_owned(),
                    text: format!("освоила «{}»", act.lesson),
                });
            } else if self.mastered[act_index] && weight < 0.5 {
                self.mastered[act_index] = false;
                events.push(LogEntry {
                    t: 0.0,
                    fly: self.id,
                    kind: "forgot".to_owned(),
                    text: format!("забыла «{}»", act.lesson),
                });
            }
        }

        // Sample the curve. Every trial would be far more data than the
        // sparkline can show, and the interesting shape is the trend.
        self.history.push_back(CurvePoint {
            trial: self.trials,
            mastery: weight.clamp(0.0, 1.0),
            weight,
            gate: self.fly.learning_gate(),
            accuracy: self.correct as f32 / self.trials.max(1) as f32,
        });
        while self.history.len() > HISTORY_LEN {
            self.history.pop_front();
        }

        self.thought = self.compose_thought(act);
        events
    }

    /// Compose the readable inner trace.
    ///
    /// This is a rendering of the brain's actual state, not a scripted line.
    /// Distress wins over learning because in the C core pain suppresses
    /// courtship and eating regardless of what the fly has learned.
    fn compose_thought(&mut self, act: &Act) -> String {
        let pain = self.fly.emotion_level(tfly::emotion::PAIN);
        let fear = self.fly.fear_level();
        let weight = self.fly.weight(act.cue, act.solution);
        let gate = self.fly.learning_gate();

        if pain > 0.25 {
            return format!(
                "боль {pain:.0}% перебивает всё — ухожу от источника (урок «{}» забыт)",
                act.lesson
            );
        }
        if fear > 0.45 {
            return format!(
                "страх {fear:.0}% доминирует, план «{}» отложен, гейт {:.0}%",
                act.lesson,
                gate * 100.0
            );
        }
        if self.fly.is_asleep() {
            return "сплю, консолидирую associations".to_owned();
        }
        if weight < 0.0 {
            return format!(
                "пробовал «{}» и ошибаюсь: вес {:.0}%, ищу новую стратегию",
                act.lesson,
                weight * 100.0
            );
        }
        if weight < 0.25 {
            return format!(
                "«{}»: нащупываю, вес {:.0}%, гейт {:.0}%, пробую чаще",
                act.lesson,
                weight * 100.0,
                gate * 100.0
            );
        }
        format!(
            "«{}»: уверенно, вес {:.0}%, гейт {:.0}%, следую плану",
            act.lesson,
            weight * 100.0,
            gate * 100.0
        )
    }
}

/// Put an act's stimulus into the fly's sensory channels.
fn present_act(fly: &mut tfly::Fly, act: &Act) {
    match act.cue {
        c if c == cue::LIGHT => {
            fly.light(0.95);
            fly.attention(tfly::sense::LIGHT, 2.0);
        }
        c if c == cue::DARK => {
            fly.light(0.02);
            fly.attention(tfly::sense::LIGHT, 2.0);
        }
        c if c == cue::VIBRATION => {
            fly.vibration(0.9);
            fly.attention(tfly::sense::VIBRATION, 2.0);
        }
        c if c == cue::SOUND => {
            fly.sound(0.9);
            fly.attention(tfly::sense::SOUND, 2.0);
        }
        c if c == cue::ODOR_FRUIT => {
            fly.odor(0.85, tfly::odor::FRUIT);
            fly.attention(tfly::sense::ODOR, 2.0);
        }
        _ => {
            fly.light(0.5);
        }
    }
    fly.attend_all(0.8);
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
            brains: (0..count)
                .map(|index| Brain::new(0x5EED + index as u32, index as u32 + 1))
                .collect(),
            acts: vec![0; count],
            current_act: 0,
            epsilon: 0.25,
            trial_interval: 0.9,
            trial_timer: 0.0,
            training: true,
            total_trials: 0,
            total_correct: 0,
            log: std::collections::VecDeque::with_capacity(LOG_LEN),
            log_file: None,
            log_path: None,
            partners: vec![None; count],
            encounter_timer: 0.0,
        }
    }

    /// Record an event, keep it in memory, and mirror it to the JSONL sink.
    fn log(&mut self, mut entry: LogEntry) {
        entry.t = self.time;
        if let Some(file) = self.log_file.as_mut()
            && let Ok(line) = serde_json::to_string(&entry)
        {
            use std::io::Write;
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
        self.log.push_back(entry);
        while self.log.len() > LOG_LEN {
            self.log.pop_front();
        }
    }

    /// The act the runtime is currently broadcasting to.
    fn current(&self) -> &'static Act {
        &ACTS[self.current_act.min(ACTS.len() - 1)]
    }

    /// Move a fly into an act and point its attention at that act's stimulus.
    fn send_to_act(&mut self, agent_index: usize, act_index: usize) {
        let act = &ACTS[act_index.min(ACTS.len() - 1)];
        self.acts[agent_index] = act_index;
        let brain = &mut self.brains[agent_index];
        brain.fly.seed(0x5EED + agent_index as u32);
        brain.fly.clear_hormones();
        present_act(&mut brain.fly, act);
    }

    /// Let characters meet when they are near each other and inclined to.
    ///
    /// A pair only meets if they are close, both off cooldown, and at least one
    /// of them actually wants company. The kind of encounter is chosen from
    /// the emotional state of both, so a frightened fly and a lonely one do
    /// the same thing and get different results.
    fn run_encounters(&mut self, dt: f32) {
        if self.agents.len() < 2 {
            return;
        }
        for i in 0..self.agents.len() {
            for j in (i + 1)..self.agents.len() {
                if !self.brains[i].fly.can_encounter() || !self.brains[j].fly.can_encounter() {
                    continue;
                }
                let (ax, ay) = (self.agents[i].position[0], self.agents[i].position[1]);
                let (bx, by) = (self.agents[j].position[0], self.agents[j].position[1]);
                let near = (ax - bx).powi(2) + (ay - by).powi(2) < 1.2;
                if !near {
                    // A lonely fly walks toward the nearest other one. This is
                    // what turns two characters standing apart into a pair that
                    // keeps running into each other.
                    let lonely =
                        self.brains[i].fly.encounter_drive() > self.brains[j].fly.encounter_drive();
                    if lonely {
                        self.agents[i].walk_toward(bx, by, dt);
                    }
                    continue;
                }
                let kind = self.choose_encounter(i, j);
                // Split the borrow so both distinct brains can be borrowed
                // mutably at once. The compiler cannot prove `i != j` from the
                // loop shape, and two simultaneous mutable borrows of one
                // field are rejected even when they are provably different
                // elements.
                let (left, right) = self.brains.split_at_mut(j);
                let meeting = tfly::meet(&mut left[i].fly, &mut right[0].fly, kind);
                if meeting.happened {
                    self.partners[i] = Some(self.agents[j].id);
                    self.partners[j] = Some(self.agents[i].id);
                    self.log(LogEntry {
                        t: 0.0,
                        fly: self.agents[i].id,
                        kind: "meet".to_owned(),
                        text: format!(
                            "#{} и #{} {}",
                            self.agents[i].id,
                            self.agents[j].id,
                            tfly::Fly::encounter_name(kind)
                        ),
                    });
                }
            }
        }
    }

    /// Choose an encounter from the emotional state of a pair.
    fn choose_encounter(&self, i: usize, j: usize) -> i32 {
        use tfly::emotion as emo;
        use tfly::encounter as enc;
        let a = &self.brains[i].fly;
        let b = &self.brains[j].fly;
        let fear = a.fear_level() + b.fear_level();
        let anger = a.emotion_level(emo::ANGER) + b.emotion_level(emo::ANGER);
        let sad = a.emotion_level(emo::SADNESS) + b.emotion_level(emo::SADNESS);
        let joy = a.emotion_level(emo::JOY) + b.emotion_level(emo::JOY);
        let trust = a.emotion_level(emo::TRUST) + b.emotion_level(emo::TRUST);
        let curious = a.emotion_level(emo::CURIOSITY) + b.emotion_level(emo::CURIOSITY);

        // An encounter is a conversation, so the loudest emotion wins and the
        // rest break the tie. That is what makes two flies do different things
        // to each other even when they are the same species.
        if fear > 1.0 {
            enc::COMFORT
        } else if anger > 0.5 {
            enc::ARGUE
        } else if sad > 0.6 {
            enc::COMFORT
        } else if trust > 1.0 {
            enc::SHARE
        } else if joy > 0.8 {
            enc::HIGH_FIVE
        } else if curious > 0.8 {
            enc::GREET
        } else {
            enc::BOW
        }
    }

    /// Run one training trial for every fly, on the act each fly is in.
    fn run_trials(&mut self) {
        for index in 0..self.agents.len() {
            let act_index = self.acts[index].min(ACTS.len() - 1);
            let act = &ACTS[act_index];
            let lesson = tfly::Lesson {
                id: act.id,
                name: act.name,
                objective: act.lesson,
                cue: act.cue,
                solution: act.solution,
                distractor: act.distractor,
            };
            let result = {
                let brain = &mut self.brains[index];
                brain.fly.steps(6, FIXED_DT);
                brain.fly.train_trial(&lesson, self.epsilon, &CANDIDATES)
            };
            let events = self.brains[index].record(act, act_index, &result, self.time);
            for event in events {
                self.log(event);
            }
            self.total_trials = self.total_trials.saturating_add(1);
            if result.outcome == tfly::TrialOutcome::Correct {
                self.total_correct = self.total_correct.saturating_add(1);
                self.learning_updates = self.learning_updates.saturating_add(1);
            }
        }
    }

    /// One walking trial: try a gait, walk, and score how well it went.
    ///
    /// The score rewards ground actually covered, penalises the energy spent,
    /// and penalises a stumble hardest, because a character that cannot stay
    /// upright is not walking at all. The write-back uses the C core's
    /// three-factor rule, so a character whose gate is shut learns nothing.
    ///
    /// Note what is being measured: distance walked, *not* progress toward the
    /// act. The character already stands on its stage, so progress toward it
    /// is always about zero and would carry no signal at all. Walking
    /// competence and knowing where to go are separate lessons, and they are
    /// trained separately.
    fn run_gait_trial(&mut self, index: usize) -> f32 {
        let start = self.agents[index].position;
        let energy_before = self.brains[index].fly.energy();

        // Pick a gait to try: mostly the learned favourite, sometimes a random
        // one so the character keeps exploring.
        let brain = &mut self.brains[index];
        let chosen = if brain.fly.random() < self.epsilon {
            brain.fly.random_index(GAIT_NAMES.len()) as i32
        } else {
            brain.fly.preferred_gait()
        };
        brain.fly.set_gait(chosen);

        // Walk for a fixed window and see what happens. The visible body is
        // advanced too, otherwise the distance measured would be zero while
        // the character was plainly walking.
        brain.fly.act(tfly::action::FORWARD, 1.0);
        brain.fly.steps(40, FIXED_DT);
        self.agents[index].advance(40, FIXED_DT);
        let energy_used = (energy_before - brain.fly.energy()).max(0.0);
        let fell = brain.fly.balance() < 0.3;

        let end = self.agents[index].position;
        let walked = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
        let score = tfly::gait_score(walked, energy_used, fell);
        // A gait that neither helps nor hurts is not evidence either way, so
        // only a real outcome is written down. Otherwise the character would
        // learn "nothing works" from a handful of neutral steps.
        if score.abs() > 0.02 {
            brain.fly.train_gait(chosen, score);
        }
        // Adopting the learned gait is what makes the change visible in the
        // rig on the next frame.
        brain.fly.set_gait(brain.fly.preferred_gait());
        score
    }

    /// Mean mastery across every fly.
    fn average_mastery(&self) -> f32 {
        if self.brains.is_empty() {
            return 0.0;
        }
        self.brains.iter().map(|b| b.mastery).sum::<f32>() / self.brains.len() as f32
    }

    /// A fly's brain, looked up by agent id.
    fn brain_of(&self, id: u32) -> Option<&Brain> {
        self.agents
            .iter()
            .position(|agent| agent.id == id)
            .and_then(|index| self.brains.get(index))
    }

    /// How strongly a fly has learned one gait, 0..1.
    fn gait_weight_for(&self, id: u32, gait: usize) -> f32 {
        self.brain_of(id)
            .map_or(0.0, |brain| brain.fly.gait_weight(gait as i32))
    }

    /// The face, straight from the brain.
    fn face_for(&self, id: u32) -> FaceSnapshot {
        let Some(brain) = self.brain_of(id) else {
            return FaceSnapshot {
                blink: 0.0,
                gaze_x: 0.0,
                gaze_y: 0.0,
                pupil: 1.0,
                brow: 0.0,
                mouth: 0.0,
                mouth_open: 0.0,
                blush: 0.0,
                tears: 0.0,
                sweat: 0.0,
            };
        };
        FaceSnapshot {
            blink: brain.fly.blink(),
            gaze_x: brain.fly.gaze_x(),
            gaze_y: brain.fly.gaze_y(),
            pupil: brain.fly.pupil(),
            brow: brain.fly.brow(),
            mouth: brain.fly.mouth(),
            mouth_open: brain.fly.mouth_open(),
            blush: brain.fly.blush(),
            tears: brain.fly.tears(),
            sweat: brain.fly.sweat(),
        }
    }

    /// Gesture and relationship state.
    fn social_for(&self, id: u32) -> SocialSnapshot {
        let Some(brain) = self.brain_of(id) else {
            return SocialSnapshot {
                gesture: "покой".to_owned(),
                strength: 0.0,
                phase: 0.0,
                bond: 0.0,
                last_encounter: String::new(),
                drive: 0.0,
                partner: None,
            };
        };
        let index = self.agents.iter().position(|agent| agent.id == id);
        SocialSnapshot {
            gesture: tfly::Fly::gesture_name(brain.fly.gesture()).to_owned(),
            strength: brain.fly.gesture_strength(),
            phase: brain.fly.gesture_phase(),
            bond: brain.fly.bond(),
            last_encounter: tfly::Fly::encounter_name(brain.fly.last_encounter()).to_owned(),
            drive: brain.fly.encounter_drive(),
            partner: index.and_then(|i| self.partners.get(i).copied().flatten()),
        }
    }

    /// Every emotion for a fly, as Russian-labelled pairs, strongest first.
    fn affect_for(&self, id: u32) -> Vec<(String, f32)> {
        let Some(brain) = self.brain_of(id) else {
            return Vec::new();
        };
        let mut pairs: Vec<(String, f32)> = brain
            .fly
            .affect()
            .into_iter()
            .map(|(emotion_id, _, value)| {
                (
                    EMOTION_NAMES
                        .get(emotion_id as usize)
                        .copied()
                        .unwrap_or("?")
                        .to_owned(),
                    value,
                )
            })
            .collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs
    }

    /// The five acts, annotated with the selected fly's progress on each.
    fn act_snapshots(&self) -> Vec<ActSnapshot> {
        let selected = self
            .selected
            .and_then(|id| self.agents.iter().position(|agent| agent.id == id))
            .or(Some(0));
        ACTS.iter()
            .map(|act| {
                let (mastery, weight) = match selected {
                    Some(si) => (
                        self.brains[si].mastery_for(act),
                        self.brains[si].fly.weight(act.cue, act.solution),
                    ),
                    None => (0.0, 0.0),
                };
                ActSnapshot {
                    id: act.id.to_owned(),
                    name: act.name.to_owned(),
                    subtitle: act.subtitle.to_owned(),
                    lesson: act.lesson.to_owned(),
                    color: act.color.to_owned(),
                    origin: act.origin,
                    cue: cue_name(act.cue).to_owned(),
                    solution: action_name(act.solution).to_owned(),
                    mastery,
                    weight,
                }
            })
            .collect()
    }

    /// Full brain readout for the selected fly.
    fn brain_snapshot(&self) -> Option<BrainSnapshot> {
        let index = self
            .selected
            .and_then(|id| self.agents.iter().position(|agent| agent.id == id))?;
        let act = &ACTS[self.acts[index].min(ACTS.len() - 1)];
        let brain = &self.brains[index];
        Some(BrainSnapshot {
            act: act.name.to_owned(),
            mood: brain.fly.dominant_emotion().to_owned(),
            drive: brain.fly.dominant_drive().to_owned(),
            thought: brain.thought.clone(),
            mastery: brain.mastery,
            trials: brain.trials,
            correct: brain.correct,
            xp: brain.xp,
            level: brain.level,
            weight: brain.fly.weight(act.cue, act.solution),
            gate: brain.fly.learning_gate(),
            plasticity: brain.fly.plasticity_level(),
            valence: brain.fly.valence(),
            arousal: brain.fly.arousal(),
            pain: brain.fly.pain_total(),
            energy: brain.fly.energy(),
            stress: brain.fly.stress(),
            memory: brain.fly.memory_count(),
            last_lesson: brain.last_lesson.to_owned(),
            last_outcome: brain.last_outcome.to_owned(),
            curve: brain.history.iter().cloned().collect(),
        })
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
        for index in 0..self.agents.len() {
            let act = &ACTS[self.acts[index].min(ACTS.len() - 1)];
            let [ax, ay] = act.origin;
            self.agents[index].step(self.time, dt, self.speed, self.decay);
            // The visible body walks with whatever the brain has settled on.
            // That is the point: the rig is not animated separately, it is a
            // readout of the C core's gait state.
            let brain = &mut self.brains[index];
            brain.fly.steps(1, FIXED_DT);
            let preferred = brain.fly.preferred_gait();
            // An untrained fly keeps whatever it was already doing.
            if brain.fly.gait_weight(preferred) > 0.01 {
                brain.fly.set_gait(preferred);
            }
            self.agents[index].gait = preferred.max(0) as usize;
            self.agents[index].gait_phase = brain.fly.gait_phase();
            self.agents[index].gait_stride = brain.fly.gait_stride();
            self.agents[index].gait_cadence = brain.fly.gait_cadence();
            self.agents[index].gait_sway = brain.fly.gait_sway();
            self.agents[index].balance = brain.fly.balance();
            self.agents[index].steps = brain.fly.step_count() as u64;
            self.agents[index].steer_towards([ax, ay], 1.2, dt * self.speed);
            self.total_reactions += 1;
            if self.agents[index].hormone_level > 0.25 {
                self.total_hormones += 1;
            }
            if self.agents[index].puff_just_started {
                self.total_puffs = self.total_puffs.saturating_add(1);
            }
            if self.adventure_id != "free_flight" {
                let dx = self.agents[index].position[0] - target[0];
                let dy = self.agents[index].position[1] - target[1];
                if (dx * dx + dy * dy).sqrt() < 0.85 {
                    self.adventure_score = self.adventure_score.saturating_add(1);
                    self.learning_updates += 1;
                    self.agents[index].reward = (self.agents[index].reward + 0.08).min(1.0);
                }
            }
        }

        // Keep every brain alive so its chemistry, drives and affect evolve
        // between trials, and present the act it is in.
        for index in 0..self.agents.len() {
            let act = &ACTS[self.acts[index].min(ACTS.len() - 1)];
            self.brains[index].fly.steps(1, FIXED_DT);
            present_act(&mut self.brains[index].fly, act);
        }

        if self.training {
            self.trial_timer += dt * self.speed;
            if self.trial_timer >= self.trial_interval {
                self.trial_timer = 0.0;
                self.run_trials();
                // Every lesson trial is followed by a walking trial, so the
                // two things the character is taught are trained together.
                for index in 0..self.agents.len() {
                    let _ = self.run_gait_trial(index);
                }
            }
        }

        // Encounters run on their own cadence, independent of the training
        // timer, so characters keep meeting even when training is paused.
        self.encounter_timer += dt * self.speed;
        if self.encounter_timer >= 0.9 {
            self.encounter_timer = 0.0;
            self.run_encounters(dt);
        }
    }

    pub fn snapshot(&self, fps: f32) -> EditorSnapshot {
        let agents = self
            .agents
            .iter()
            .map(|agent| {
                let (drive, decision, confidence) = agent.intent();
                let weights: Vec<(String, f32)> = GAIT_NAMES
                    .iter()
                    .enumerate()
                    .map(|(index, name)| (name.to_string(), self.gait_weight_for(agent.id, index)))
                    .collect();
                let mastery = weights.iter().map(|(_, w)| *w).fold(0.0f32, f32::max);
                let affect = self.affect_for(agent.id);
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
                    height: 0.0,
                    gait: GaitSnapshot {
                        preset: GAIT_NAMES[agent.gait.min(GAIT_NAMES.len() - 1)].to_owned(),
                        phase: agent.gait_phase,
                        stride: agent.gait_stride,
                        cadence: agent.gait_cadence,
                        sway: agent.gait_sway,
                        balance: agent.balance,
                        steps: agent.steps,
                        weights: weights.clone(),
                        mastery,
                    },
                    affect,
                    face: self.face_for(agent.id),
                    social: self.social_for(agent.id),
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
                    progress: if self.adventure_id == "free_flight" {
                        1.0
                    } else {
                        (self.adventure_score % 100) as f32 / 100.0
                    },
                    score: self.adventure_score,
                    target: self.adventure_target,
                }
            },
            acts: self.act_snapshots(),
            training: TrainingSnapshot {
                enabled: self.training,
                epsilon: self.epsilon,
                interval: self.trial_interval,
                current_act: self.current_act,
                total_trials: self.total_trials,
                total_correct: self.total_correct,
                average_mastery: self.average_mastery(),
                log: self.log.iter().cloned().collect(),
                log_path: self
                    .log_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
            },
            brain: self.brain_snapshot(),
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
            model_note: format!(
                "TFLY.h core, three-factor learning. Broadcast act: {} ({}).",
                self.current().name,
                self.current().lesson
            ),
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
            // Send flies into one of the five acts. Without an id, every fly
            // goes; with an id, only that fly does.
            "act" => {
                let requested = command.name.context("act requires name")?;
                let act = act_by_id(&requested).context("unknown act")?;
                let act_index = ACTS
                    .iter()
                    .position(|candidate| candidate.id == act.id)
                    .unwrap_or(0);
                self.current_act = act_index;
                self.adventure_id = "free_flight".to_owned();
                self.update_adventure_target();
                match command.id {
                    Some(id) => {
                        if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                            self.send_to_act(index, act_index);
                        }
                    }
                    None => {
                        for index in 0..self.agents.len() {
                            self.send_to_act(index, act_index);
                        }
                    }
                }
                self.log(LogEntry {
                    t: 0.0,
                    fly: 0,
                    kind: "act".to_owned(),
                    text: format!("все на сцене: {}", act.name),
                });
            }
            // Train the selected fly hard in its current act, for a burst of
            // trials without waiting for the timer.
            "train" => {
                let burst = (command.value.unwrap_or(40.0) as usize).clamp(1, 2000);
                let index = self
                    .selected
                    .and_then(|id| self.agents.iter().position(|agent| agent.id == id))
                    .or(Some(0));
                if let Some(index) = index {
                    let act_index = self.acts[index];
                    let act = &ACTS[act_index];
                    let lesson = tfly::Lesson {
                        id: act.id,
                        name: act.name,
                        objective: act.lesson,
                        cue: act.cue,
                        solution: act.solution,
                        distractor: act.distractor,
                    };
                    for _ in 0..burst {
                        let result =
                            self.brains[index]
                                .fly
                                .train_trial(&lesson, self.epsilon, &CANDIDATES);
                        let events = self.brains[index].record(act, act_index, &result, self.time);
                        for event in events {
                            self.log(event);
                        }
                        self.total_trials = self.total_trials.saturating_add(1);
                        if result.outcome == tfly::TrialOutcome::Correct {
                            self.total_correct = self.total_correct.saturating_add(1);
                            self.learning_updates = self.learning_updates.saturating_add(1);
                        }
                    }
                    self.log(LogEntry {
                        t: 0.0,
                        fly: self.brains[index].id,
                        kind: "train".to_owned(),
                        // The level is stated here because the per-trial
                        // level-up lines are rate limited and a burst may
                        // cross many levels at once.
                        text: format!(
                            "тренировка ×{burst} в «{}» → уровень {}",
                            act.name, self.brains[index].level
                        ),
                    });
                }
            }
            "training" => {
                self.training = command.enabled.unwrap_or(!self.training);
                self.log(LogEntry {
                    t: 0.0,
                    fly: 0,
                    kind: "training".to_owned(),
                    text: if self.training {
                        "автообучение включено".to_owned()
                    } else {
                        "автообучение выключено".to_owned()
                    },
                });
            }
            "epsilon" => {
                self.epsilon = command.value.unwrap_or(self.epsilon).clamp(0.0, 1.0);
            }
            // Send a pain signal to a fly, for teaching avoidance.
            "hurt" => {
                let id = command.id.context("hurt requires id")?;
                let intensity = command.value.unwrap_or(0.7).clamp(0.0, 1.0);
                if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                    self.brains[index].fly.heart(intensity, tfly::body::WING_L);
                    self.brains[index].fly.steps(2, FIXED_DT);
                    self.brains[index].thought = self.brains[index]
                        .compose_thought(&ACTS[self.acts[index].min(ACTS.len() - 1)]);
                    self.log(LogEntry {
                        t: 0.0,
                        fly: id,
                        kind: "pain".to_owned(),
                        text: format!("боль {:.0}% в левое крыло", intensity * 100.0),
                    });
                }
            }
            // Hormone administration. With `name` it injects into the C brain,
            // e.g. dopamine to open the learning gate. With `enabled` it toggles
            // the fly's own endocrine channel in the editor.
            "hormone" => {
                let id = command.id.context("hormone requires id")?;
                if let Some(name) = command.name.as_deref() {
                    let intensity = command.value.unwrap_or(0.5).clamp(0.0, 1.0);
                    if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                        self.brains[index].fly.hormone_named(intensity, name, 5.0);
                        self.log(LogEntry {
                            t: 0.0,
                            fly: id,
                            kind: "hormone".to_owned(),
                            text: format!("{name} {:.0}%", intensity * 100.0),
                        });
                    }
                } else if let Some(enabled) = command.enabled
                    && let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id)
                {
                    agent.hormone_enabled = enabled;
                }
            }
            // Wipe a fly's learning, keeping its body state.
            "unlearn" => {
                let id = command.id.context("unlearn requires id")?;
                if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                    let brain = &mut self.brains[index];
                    brain.fly.reset_learning();
                    brain.fly.forget();
                    brain.mastery = 0.0;
                    brain.xp = 0;
                    brain.level = 1;
                    brain.trials = 0;
                    brain.correct = 0;
                    brain.history.clear();
                    brain.mastered = [false; 5];
                    self.log(LogEntry {
                        t: 0.0,
                        fly: id,
                        kind: "unlearn".to_owned(),
                        text: "ассоциации стёрты".to_owned(),
                    });
                }
            }
            // Train the character to walk. This is a different mechanism from
            // the lesson trials: the fly tries a gait and is scored on how far
            // it got, not on whether it picked the right action.
            "walk" => {
                let burst = (command.value.unwrap_or(30.0) as usize).clamp(1, 2000);
                let index = self
                    .selected
                    .and_then(|id| self.agents.iter().position(|agent| agent.id == id))
                    .or(Some(0));
                if let Some(index) = index {
                    for _ in 0..burst {
                        let _ = self.run_gait_trial(index);
                    }
                    let brain = &self.brains[index];
                    let best = brain.fly.preferred_gait();
                    let mastery = brain.fly.gait_weight(best);
                    self.log(LogEntry {
                        t: 0.0,
                        fly: brain.id,
                        kind: "walk".to_owned(),
                        text: format!(
                            "ходьба ×{burst}, походка «{}» освоена на {:.0}%",
                            GAIT_NAMES[best.max(0) as usize],
                            mastery * 100.0
                        ),
                    });
                }
            }
            // Force one specific gait, to compare them by hand.
            "gait" => {
                let id = command.id.context("gait requires id")?;
                let which = command.value.unwrap_or(1.0).clamp(0.0, 3.0) as i32;
                if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                    let brain = &mut self.brains[index];
                    brain.fly.set_gait(which);
                    self.agents[index].gait = which.max(0) as usize;
                    self.agents[index].gait_stride = brain.fly.gait_stride();
                    self.agents[index].gait_cadence = brain.fly.gait_cadence();
                    self.agents[index].gait_sway = brain.fly.gait_sway();
                }
            }
            // Make two flies meet on demand, whatever their feelings say.
            "meet" => {
                let first = command.id.context("meet requires the first fly id")?;
                let second = command.value.map(|v| v as u32);
                let a = self.agents.iter().position(|agent| agent.id == first);
                let b = match second {
                    Some(id) => self.agents.iter().position(|agent| agent.id == id),
                    None => self
                        .agents
                        .iter()
                        .enumerate()
                        .find(|(i, agent)| agent.id != first && self.brains[*i].fly.can_encounter())
                        .map(|(i, _)| i),
                };
                if let (Some(a), Some(b)) = (a, b) {
                    let kind = self.choose_encounter(a, b);
                    // Split the borrow so both distinct brains are mutable.
                    let (left, right) = self.brains.split_at_mut(b);
                    let meeting = tfly::meet(&mut left[a].fly, &mut right[0].fly, kind);
                    if meeting.happened {
                        self.partners[a] = Some(self.agents[b].id);
                        self.partners[b] = Some(self.agents[a].id);
                        self.log(LogEntry {
                            t: 0.0,
                            fly: self.agents[a].id,
                            kind: "meet".to_owned(),
                            text: format!(
                                "#{} и #{} {}",
                                self.agents[a].id,
                                self.agents[b].id,
                                tfly::Fly::encounter_name(kind)
                            ),
                        });
                    } else {
                        self.log(LogEntry {
                            t: 0.0,
                            fly: self.agents[a].id,
                            kind: "meet".to_owned(),
                            text: "не сейчас, кулдаун".to_owned(),
                        });
                    }
                }
            }
            // Make a character play a pose by hand.
            "gesture" => {
                let id = command.id.context("gesture requires id")?;
                let which = command.value.unwrap_or(1.0).clamp(0.0, 7.0) as i32;
                if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                    // Forced, because this is a button press: silently dropping
                    // the request because a previous pose is still finishing
                    // would look like a broken control.
                    self.brains[index].fly.force_gesture(which);
                    self.log(LogEntry {
                        t: 0.0,
                        fly: id,
                        kind: "gesture".to_owned(),
                        text: tfly::Fly::gesture_name(which).to_owned(),
                    });
                }
            }
            // Start or stop mirroring events to a JSONL file.
            "logfile" => {
                let path = command.name.clone();
                match path {
                    Some(name) if !name.is_empty() && name != "off" => {
                        let file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&name)
                            .context("opening log file")?;
                        self.log_path = Some(std::path::PathBuf::from(&name));
                        self.log_file = Some(file);
                        self.log(LogEntry {
                            t: 0.0,
                            fly: 0,
                            kind: "logfile".to_owned(),
                            text: format!("лог открыт: {name}"),
                        });
                    }
                    _ => {
                        self.log_file = None;
                        self.log_path = None;
                        self.log(LogEntry {
                            t: 0.0,
                            fly: 0,
                            kind: "logfile".to_owned(),
                            text: "лог закрыт".to_owned(),
                        });
                    }
                }
            }
            "hormone_toggle" => {
                let id = command.id.context("hormone_toggle requires id")?;
                let enabled = command.enabled.unwrap_or(true);
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.hormone_enabled = enabled;
                }
            }
            "add" => {
                if self.agents.len() < MAX_FLIES {
                    let id = self.agents.iter().map(|agent| agent.id).max().unwrap_or(0) + 1;
                    self.agents.push(Agent::new(id, self.agents.len()));
                    self.brains.push(Brain::new(0x5EED + id, id));
                    self.acts.push(self.current_act);
                    self.selected = Some(id);
                }
            }
            "remove" => {
                let id = command.id.context("remove requires id")?;
                if self.agents.len() > 1 {
                    if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                        self.agents.remove(index);
                        self.brains.remove(index);
                        self.acts.remove(index);
                    }
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
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store, must-revalidate\r\nPragma: no-cache\r\nConnection: keep-alive\r\n\r\n{body}",
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
    fn there_are_exactly_five_acts_and_they_are_distinct() {
        assert_eq!(ACTS.len(), 5);
        let mut ids: Vec<&str> = ACTS.iter().map(|act| act.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "act ids must be unique");

        // Two acts sharing a cue could not be discriminated by the fly, so a
        // duplicate cue would silently break the curriculum.
        let mut cues: Vec<i32> = ACTS.iter().map(|act| act.cue).collect();
        cues.sort_unstable();
        let before = cues.len();
        cues.dedup();
        assert_eq!(cues.len(), before, "each act needs its own cue");

        for act in ACTS {
            assert!(
                CANDIDATES.contains(&act.solution),
                "act {} must have a solvable action",
                act.id
            );
            assert!(
                CANDIDATES.contains(&act.distractor),
                "act {} must have a distractor among the candidates",
                act.id
            );
            assert_ne!(act.solution, act.distractor);
        }
    }

    #[test]
    fn send_command_moves_every_fly_to_the_chosen_act() {
        let mut runtime = EditorRuntime::new(3);
        for (index, act) in ACTS.iter().enumerate() {
            runtime
                .apply_command(format!(r#"{{"action":"act","name":"{}"}}"#, act.id).as_bytes())
                .expect("act command");
            assert!(runtime.acts.iter().all(|a| *a == index));
        }
        let snapshot = runtime.snapshot(60.0);
        assert_eq!(snapshot.acts.len(), 5);
        assert_eq!(snapshot.training.current_act, 4);
    }

    #[test]
    fn send_command_can_target_a_single_fly() {
        let mut runtime = EditorRuntime::new(3);
        runtime
            .apply_command(br#"{"action":"act","name":"void","id":2}"#)
            .expect("act command");
        assert_eq!(runtime.acts[1], 4, "fly 2 moved");
        assert_eq!(runtime.acts[0], 0, "fly 1 stayed");
        assert_eq!(runtime.acts[2], 0, "fly 3 stayed");
    }

    #[test]
    fn training_improves_mastery_of_the_current_act() {
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"garden"}"#)
            .expect("act command");
        let before = runtime.brains[0].mastery;
        // A burst of trials in one act must move the fly's real associative
        // weight for that act's lesson.
        runtime
            .apply_command(br#"{"action":"train","value":600}"#)
            .expect("train command");
        let after = runtime.brains[0].mastery;
        let act = &ACTS[2];
        let weight = runtime.brains[0].fly.weight(act.cue, act.solution);
        assert!(
            after > before,
            "mastery must rise with training: {before} -> {after}"
        );
        assert!(
            weight > 0.2,
            "the fly must actually learn the garden lesson, weight={weight}"
        );
    }

    #[test]
    fn unlearn_wipes_the_brain_but_keeps_the_body() {
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"train","value":200}"#)
            .expect("train");
        assert!(runtime.brains[0].xp > 0);
        runtime
            .apply_command(br#"{"action":"unlearn","id":1}"#)
            .expect("unlearn");
        let brain = &runtime.brains[0];
        assert_eq!(brain.xp, 0);
        assert_eq!(brain.level, 1);
        assert_eq!(brain.trials, 0);
        for act in ACTS {
            assert_eq!(brain.fly.weight(act.cue, act.solution), 0.0);
        }
    }

    #[test]
    fn pain_overrides_a_learned_lesson_and_shows_in_the_trace() {
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"train","value":400}"#)
            .expect("train");
        runtime
            .apply_command(br#"{"action":"hurt","id":1,"value":0.9}"#)
            .expect("hurt");
        let snapshot = runtime.snapshot(60.0);
        let brain = snapshot.brain.expect("brain snapshot");
        assert!(brain.pain > 0.1, "pain must register: {}", brain.pain);
        assert!(
            brain.thought.contains("боль"),
            "the trace must mention pain, got: {}",
            brain.thought
        );
    }

    #[test]
    fn adding_and_removing_a_fly_keeps_brains_aligned() {
        let mut runtime = EditorRuntime::new(2);
        runtime.apply_command(br#"{"action":"add"}"#).expect("add");
        assert_eq!(runtime.agents.len(), runtime.brains.len());
        assert_eq!(runtime.agents.len(), runtime.acts.len());
        runtime
            .apply_command(br#"{"action":"remove","id":2}"#)
            .expect("remove");
        assert_eq!(runtime.agents.len(), 2);
        assert_eq!(runtime.brains.len(), 2);
        assert_eq!(runtime.acts.len(), 2);
    }

    #[test]
    fn mastery_starts_at_zero_and_reaches_one() {
        let mut runtime = EditorRuntime::new(1);
        // An untouched fly must read as no mastery at all, not as a midpoint.
        for act in ACTS {
            assert_eq!(
                runtime.brains[0].mastery_for(&act),
                0.0,
                "act {} should start at 0% mastery",
                act.id
            );
        }
        // After heavy training the lesson should saturate at full mastery,
        // rather than getting stuck at half because of a bad rescale.
        runtime
            .apply_command(br#"{"action":"act","name":"void"}"#)
            .expect("act");
        runtime
            .apply_command(br#"{"action":"train","value":1500}"#)
            .expect("train");
        let mastery = runtime.brains[0].mastery_for(&ACTS[4]);
        assert!(
            mastery > 0.9,
            "a fully trained act must read near 100%, got {mastery}"
        );
    }

    #[test]
    fn flies_actually_fly_to_the_act_they_are_sent_to() {
        let mut runtime = EditorRuntime::new(3);
        runtime
            .apply_command(br#"{"action":"act","name":"void"}"#)
            .expect("act");
        // Start the troupe far from the void so the test proves convergence
        // rather than coincidence.
        for agent in &mut runtime.agents {
            agent.position = [0.0, 0.0, 1.2];
        }
        for _ in 0..900 {
            runtime.step(FIXED_DT);
        }
        let [gx, gy] = ACTS[4].origin;
        for agent in &runtime.agents {
            let dx = agent.position[0] - gx;
            let dy = agent.position[1] - gy;
            let distance = (dx * dx + dy * dy).sqrt();
            assert!(
                distance < 2.0,
                "fly {} should reach the void, distance {distance}",
                agent.id
            );
        }
    }

    #[test]
    fn troupe_spreads_out_instead_of_stacking() {
        let mut runtime = EditorRuntime::new(3);
        runtime
            .apply_command(br#"{"action":"act","name":"garden"}"#)
            .expect("act");
        for agent in &mut runtime.agents {
            agent.position = [0.0, 0.0, 1.2];
        }
        for _ in 0..900 {
            runtime.step(FIXED_DT);
        }
        let a = &runtime.agents[0].position;
        let b = &runtime.agents[1].position;
        let separation = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        assert!(
            separation > 0.3,
            "flies sharing a stage must not collapse onto one point, got {separation}"
        );
    }

    #[test]
    fn the_log_records_progress_and_bumps() {
        let mut runtime = EditorRuntime::new(1);
        // Start from a clean slate so the act command produces a line.
        assert!(runtime.log.is_empty());
        runtime
            .apply_command(br#"{"action":"act","name":"garden"}"#)
            .expect("act");
        assert_eq!(runtime.log.len(), 1);
        assert_eq!(runtime.log[0].kind, "act");

        runtime
            .apply_command(br#"{"action":"train","value":400}"#)
            .expect("train");
        let kinds: Vec<&str> = runtime.log.iter().map(|e| e.kind.as_str()).collect();
        assert!(
            kinds.contains(&"mastered"),
            "mastering a lesson must be logged, got {kinds:?}"
        );
        assert!(
            kinds.contains(&"train"),
            "the training burst must be logged"
        );
        assert!(
            kinds.contains(&"level_up"),
            "crossing a level must be logged"
        );

        // The curve must be populated for the sparkline.
        let snapshot = runtime.snapshot(60.0);
        let brain = snapshot.brain.expect("brain");
        assert!(brain.curve.len() > 10, "curve should have samples");
        assert!(brain.curve.iter().all(|p| (0.0..=1.0).contains(&p.mastery)));
    }

    #[test]
    fn forgetting_is_logged_and_clears_the_curve() {
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"train","value":300}"#)
            .expect("train");
        assert!(!runtime.brains[0].history.is_empty());
        runtime
            .apply_command(br#"{"action":"unlearn","id":1}"#)
            .expect("unlearn");
        assert!(runtime.brains[0].history.is_empty());
        assert!(
            runtime.log.iter().any(|e| e.kind == "unlearn"),
            "wiping the brain must be logged"
        );
    }

    #[test]
    fn log_can_be_mirrored_to_a_jsonl_file() {
        let mut runtime = EditorRuntime::new(1);
        let path = std::env::temp_dir().join("flytest-editor-log-test.jsonl");
        let _ = std::fs::remove_file(&path);
        let name = path.display().to_string();
        // Build the command as JSON rather than formatting it by hand, so a
        // Windows path with backslashes cannot corrupt the payload.
        let command = serde_json::json!({ "action": "logfile", "name": name });
        runtime
            .apply_command(command.to_string().as_bytes())
            .expect("logfile on");
        runtime
            .apply_command(br#"{"action":"act","name":"void"}"#)
            .expect("act");
        assert!(path.exists(), "log file must be created");
        let body = std::fs::read_to_string(&path).expect("read log");
        let lines: Vec<&str> = body.lines().filter(|l| !l.is_empty()).collect();
        assert!(lines.len() >= 2, "expected several JSONL lines");
        for line in &lines {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("every line must be valid JSON");
            assert!(parsed.get("t").is_some());
            assert!(parsed.get("kind").is_some());
            assert!(parsed.get("text").is_some());
        }
        runtime
            .apply_command(br#"{"action":"logfile","name":"off"}"#)
            .expect("logfile off");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn level_up_lines_are_rate_limited() {
        let mut runtime = EditorRuntime::new(1);
        // A single burst crosses many levels within one frame. Every one of
        // them being logged would bury the rest of the journal.
        runtime
            .apply_command(br#"{"action":"train","value":900}"#)
            .expect("train");
        let level_lines = runtime.log.iter().filter(|e| e.kind == "level_up").count();
        assert!(
            level_lines <= 2,
            "a same-frame burst must not log every level, got {level_lines}"
        );
        // The burst itself reports the level actually reached, so rate
        // limiting the per-trial lines does not lose that information.
        let level = runtime.brains[0].level;
        assert!(
            level > 10,
            "the burst should have levelled the fly up, got {level}"
        );
        assert!(
            runtime
                .log
                .iter()
                .any(|e| e.kind == "train" && e.text.contains(&format!("уровень {level}"))),
            "the training line must state the level actually reached ({level})"
        );
    }

    #[test]
    fn characters_walk_and_learn_a_gait() {
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"garden"}"#)
            .expect("act");
        for _ in 0..600 {
            runtime.step(FIXED_DT);
        }
        let agent = &runtime.agents[0];
        assert!(agent.steps > 0, "a walking character must count steps");
        assert!(
            agent.gait_phase > 0.0,
            "the walk cycle must advance, phase={}",
            agent.gait_phase
        );
        // Training must shift the learned gait weights away from zero.
        runtime
            .apply_command(br#"{"action":"walk","value":300}"#)
            .expect("walk");
        let preferred = runtime.brains[0].fly.preferred_gait();
        let best = runtime.brains[0].fly.gait_weight(preferred);
        assert!(
            best > 0.05,
            "walking trials must teach a gait, best weight={best}"
        );
        assert!(
            runtime.log.iter().any(|e| e.kind == "walk"),
            "walk training must be logged"
        );
    }

    #[test]
    fn characters_stay_on_the_ground() {
        let mut runtime = EditorRuntime::new(2);
        for _ in 0..900 {
            runtime.step(FIXED_DT);
        }
        for agent in &runtime.agents {
            // Position index 2 used to be altitude when these things flew. It
            // must now stay pinned to the floor.
            assert_eq!(
                agent.position[2], 0.0,
                "characters must not leave the ground"
            );
            assert_eq!(agent.velocity[2], 0.0, "there must be no vertical velocity");
        }
    }

    #[test]
    fn the_snapshot_exposes_gait_and_the_full_affect_set() {
        let mut runtime = EditorRuntime::new(1);
        for _ in 0..120 {
            runtime.step(FIXED_DT);
        }
        let snapshot = runtime.snapshot(60.0);
        let agent = &snapshot.agents[0];
        assert_eq!(agent.affect.len(), EMOTION_NAMES.len());
        assert!(
            agent
                .affect
                .iter()
                .all(|(name, _)| EMOTION_NAMES.contains(&name.as_str())),
            "every affect label must be a known emotion"
        );
        for pair in agent.affect.windows(2) {
            assert!(pair[0].1 >= pair[1].1, "affect must be sorted descending");
        }
        assert_eq!(agent.gait.weights.len(), GAIT_NAMES.len());
        assert!(agent.gait.steps > 0);
    }

    #[test]
    fn characters_meet_and_build_bonds() {
        let mut runtime = EditorRuntime::new(3);
        // Put them all at the same act so they end up close together.
        runtime
            .apply_command(br#"{"action":"act","name":"main_stage"}"#)
            .expect("act");
        // Run long enough for the social layer to fire repeatedly.
        for _ in 0..4000 {
            runtime.step(FIXED_DT);
        }
        let bonds: Vec<f32> = runtime
            .brains
            .iter()
            .map(|brain| brain.fly.bond())
            .collect();
        assert!(
            bonds.iter().any(|b| *b > 0.0),
            "characters that keep meeting must form a bond: {bonds:?}"
        );
        // A partner must have been recorded.
        assert!(
            runtime.partners.iter().any(|p| p.is_some()),
            "the pair must be tracked as partners"
        );
        // And the meeting must be in the log.
        assert!(
            runtime.log.iter().any(|e| e.kind == "meet"),
            "meetings must be logged"
        );
    }

    #[test]
    fn a_wounded_character_meets_more_cautiously() {
        let mut runtime = EditorRuntime::new(2);
        runtime
            .apply_command(br#"{"action":"act","name":"main_stage"}"#)
            .expect("act");
        for _ in 0..2000 {
            runtime.step(FIXED_DT);
        }
        let calm_drive = runtime.brains[0].fly.encounter_drive();
        // Pain and fear should suppress the urge to socialise.
        runtime
            .apply_command(br#"{"action":"hurt","id":1,"value":0.9}"#)
            .expect("hurt");
        runtime.brains[0].fly.steps(30, FIXED_DT);
        let hurt_drive = runtime.brains[0].fly.encounter_drive();
        assert!(
            hurt_drive < calm_drive,
            "a frightened fly must want company less: calm={calm_drive} hurt={hurt_drive}"
        );
    }

    #[test]
    fn the_snapshot_carries_the_face() {
        let mut runtime = EditorRuntime::new(1);
        for _ in 0..300 {
            runtime.step(FIXED_DT);
        }
        let agent = &runtime.snapshot(60.0).agents[0];
        assert!((0.0..=1.0).contains(&agent.face.blink));
        assert!((-1.0..=1.0).contains(&agent.face.gaze_x));
        assert!((0.5..=1.6).contains(&agent.face.pupil));
        assert!((-1.0..=1.0).contains(&agent.face.mouth));
        assert!(!agent.social.gesture.is_empty());
    }

    #[test]
    fn unknown_act_is_rejected() {
        let mut runtime = EditorRuntime::new(1);
        assert!(
            runtime
                .apply_command(br#"{"action":"act","name":"moon_base"}"#)
                .is_err()
        );
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
