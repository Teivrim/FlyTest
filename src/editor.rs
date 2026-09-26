use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::course::Course;
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

/// One emotion, as reported to the client.
#[derive(Debug, Clone, Serialize)]
pub struct EmotionReading {
    /// Russian label, for the panel.
    pub name: String,
    /// Stable ASCII key, for the rig.
    pub key: &'static str,
    /// 0..1.
    pub value: f32,
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
    /// How far she has chosen to open her eyes, 0..1. The lid is shut if this
    /// or the reflex wants it shut.
    pub eye_open: f32,
    /// Pupil adaptation, 0 dark .. 1 bright.
    pub eye_adapt: f32,
    /// Seconds since she woke, which the rig uses for the waking sequence.
    pub wake_timer: f32,
}

/// How she carries her body, as opposed to how her face looks.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PostureSnapshot {
    /// -1 curled in .. 1 stretched up.
    pub spine: f32,
    /// 0 down .. 1 shrugged up.
    pub shoulder: f32,
    /// -1 leaning back .. 1 leaning forward.
    pub lean: f32,
}

/// The four joints of one limb, in radians.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct JointsSnapshot {
    /// Hip or shoulder swing.
    pub root: f32,
    /// Knee or elbow bend.
    pub middle: f32,
    /// Ankle or wrist.
    pub end: f32,
    /// Sideways splay.
    pub spread: f32,
}

/// Every joint of every limb, so the rig can pose four chains independently.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LimbSnapshot {
    pub leg_l: JointsSnapshot,
    pub leg_r: JointsSnapshot,
    pub arm_l: JointsSnapshot,
    pub arm_r: JointsSnapshot,
    /// How far below the hip the lower foot reaches, given the rig's segment
    /// lengths. The rig subtracts this from the root to stand on the floor.
    pub foot_drop: f32,
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
    /// Height of the surface she is standing on, so the rig stands her on the
    /// stage instead of inside it.
    pub ground: f32,
    pub gait: GaitSnapshot,
    /// Every emotion, not just the dominant one, for the affect display.
    ///
    /// Each entry is a display name, a value, and a stable ASCII key. The key
    /// is what the rig reads: matching on the Russian label would break the
    /// moment a name is reworded, and matching on the index would break the
    /// moment an emotion is inserted.
    pub affect: Vec<EmotionReading>,
    /// The face, for the rig.
    pub face: FaceSnapshot,
    /// How she carries her body.
    pub posture: PostureSnapshot,
    /// Every joint of every limb.
    pub limbs: LimbSnapshot,
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

/// One wall of the course, as the client draws it.
///
/// The stage's own posts are circles and are not listed here: the client already
/// draws them from the act's colliders. These are the maze, and they are sent in
/// stage-local coordinates so they move with the stage the way the disc does.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WallSnapshot {
    pub x: f32,
    pub z: f32,
    pub half_len: f32,
    pub half_thick: f32,
    pub angle: f32,
}

/// What a character is doing on the course right now.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerSnapshot {
    pub id: u32,
    /// She has reached the food this round.
    pub fed: bool,
    /// The deepest food she reached, counting from the entrance, or none.
    pub ate_feed: usize,
    /// How many of the maze's foods she has had.
    pub ate_count: u32,
    /// How many there are.
    pub feeds_total: u32,
    /// How far she is from the food, in stage-local units.
    pub distance: f32,
    /// How much of the way she has come along the route, 0..1. Straight-line
    /// distance is the honest measure; she does not know the route, so the panel
    /// does not pretend she is following one.
    pub progress: f32,
    /// Rounds she has finished.
    pub score: u32,
    /// Where she has been this round, stage-local, for drawing and for the others
    /// to follow.
    pub trail: Vec<[f32; 2]>,
}

/// The course, as the client needs it.
#[derive(Debug, Clone, Serialize)]
pub struct CourseSnapshot {
    /// Which act this is, so the client can find the stage to put it on.
    ///
    /// All five acts are sent, not just the one being broadcast. They are all on
    /// screen at once, and a maze on one stage and bare floor on the other four
    /// reads as a bug rather than as a choice.
    pub act: usize,
    /// Which round this is, counted from one.
    pub round: u32,
    /// The seed the maze was built from, so a round can be repeated.
    pub seed: u32,
    /// Seconds left before the course is rebuilt under them.
    pub time_left: f32,
    /// Seconds a round lasts.
    pub round_length: f32,
    /// The food this round is played for: the first one, which is the nearest.
    pub food: [f32; 2],
    /// Every food in the maze, nearest the entrance first.
    ///
    /// All of them are drawn, and the one a character reaches disappears. A maze
    /// of a hundred rooms with a single prize at the far end is not a round, it is
    /// a march, and measuring it showed nothing arriving in any time a circus can
    /// sit through. So a big maze has several, and how deep she got is the thing
    /// worth watching.
    pub feeds: Vec<[f32; 2]>,
    /// Side of one maze cell, so the client can size its own marks to the course
    /// instead of guessing.
    pub cell: f32,
    /// Where the characters start, stage-local.
    pub start: [f32; 2],
    /// Length of the way through, for scoring.
    pub path_length: f32,
    /// The maze.
    pub walls: Vec<WallSnapshot>,
    /// Every character on this course.
    pub runners: Vec<RunnerSnapshot>,
    /// How many have eaten.
    pub fed: u32,
    /// How many there are.
    pub count: u32,
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

/// Half the distance two comfortable characters will tolerate between them.
/// Shyness and fear widen it, a bond narrows it.
const PERSONAL_SPACE: f32 = 0.55;

/// The rig's leg dimensions. They belong to the renderer, but the model needs
/// them to place the body on the floor: it owns the joint angles, and only the
/// renderer knows how long the bones are.
///
/// The sole is an ellipsoid, not a point. A point below the ankle overestimates
/// the foot's height by most of a foot as the ankle tilts, which leaves the
/// character hovering whenever the knee folds.
/// How wide a character is for the purpose of walking past scenery. She is a
/// person, not a point, and a point walks through a wall without noticing.
pub const BODY_RADIUS: f32 = 0.18;

/// How far from the stage centre a character can stand without being inside
/// something.
///
/// This is derived from the colliders rather than written down, because the
/// number that matters is the one the geometry actually implies. A separate
/// hand-tuned value would agree with the scenery right up until somebody moved
/// a wall.
pub fn clear_centre(act: &Act) -> f32 {
    let mut inner = act.stage_radius;
    for c in act.colliders {
        let r = (c.x * c.x + c.z * c.z).sqrt();
        if r <= 0.0 {
            continue;
        }
        // The inner edge of this solid, measured along its own radius.
        let edge = r - c.radius;
        if edge > 0.0 && edge < inner {
            inner = edge;
        }
    }
    inner
}

/// The largest circle a character can mill around on this stage without
/// grinding into its scenery.
///
/// The margin is not decoration. Clamping the orbit to exactly the limit puts
/// her permanently in contact with the scenery, and a character in contact
/// with a wall while the stage pulls her inward grinds rather than walks
/// around, which is the thing the clamp was meant to prevent.
pub fn orbit_room(act: &Act) -> f32 {
    (clear_centre(act) - BODY_RADIUS - ORBIT_MARGIN).max(0.0)
}

/// How much clear space to leave between a milling character and the scenery.
pub const ORBIT_MARGIN: f32 = 0.10;

/// Leg segment lengths, which the model's foot-drop calculation needs but does
/// not know. They are the renderer's proportions; the model supplies the angles
/// and is told how long the bones are.
pub const RIG_LEG: tfly::Sole = tfly::Sole {
    thigh: 0.10,
    shin: 0.10,
    sole_c: 0.017,
    sole_a: 0.032 * 0.6,
    sole_b: 0.032 * 1.35,
    sole_f: 0.020,
};
#[derive(Debug, Clone, Serialize)]
pub struct BrainSnapshot {
    pub act: String,
    /// The association actually being trained, stated in words. "What is she
    /// learning on" was unanswerable from a percentage and a mood.
    pub learning: String,
    pub mood: String,
    pub drive: String,
    pub thought: String,
    /// The association weight, 0..1. This is what "learned" means.
    pub mastery: f32,
    /// Smoothed share of trials answered correctly. Performance, not learning.
    pub hit_rate: f32,
    pub trials: u32,
    pub correct: u32,
    pub xp: u32,
    pub level: u32,
    pub weight: f32,
    pub gate: f32,
    /// The modulator's terms, so a low gate can be read rather than guessed at.
    pub gate_terms: Vec<GateTerm>,
    /// Trials left to reach a weight of 0.9, measured. `None` while the weight
    /// is not moving, which is the case worth explaining.
    pub trials_left: Option<u32>,
    /// The learning rate in force, as a multiple of the core's default.
    pub learn_rate: f32,
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

/// One term of the modulator, with what it is currently contributing.
#[derive(Debug, Clone, Serialize)]
pub struct GateTerm {
    pub key: String,
    pub name: String,
    /// Signed contribution to the gate, in the gate's own units.
    pub contribution: f32,
    /// The level this term is read from, 0..1.
    pub level: f32,
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
    /// Radius of the walkable stage top.
    pub stage_radius: f32,
    /// How high the stage stands above the tent floor.
    pub stage_height: f32,
    /// Solids on the stage, in act-local coordinates.
    ///
    /// The client builds the stage geometry from these numbers rather than
    /// keeping its own copy. Two sets of numbers is two chances to put a wall
    /// in one place and its collider in another, and the bug that follows is
    /// invisible until someone walks through a wall.
    pub colliders: &'static [Collider],
    /// Mastery of the selected fly on this act, 0..1.
    pub mastery: f32,
    /// How strongly the fly currently associates the cue with the solution.
    pub weight: f32,
}

impl EditorSnapshot {
    /// The course for the act currently being broadcast.
    ///
    /// The snapshot carries one course per act, because they are all on screen at
    /// once, but the panel is about one of them and this says which.
    pub fn shown_course(&self) -> &CourseSnapshot {
        let act = self
            .training
            .current_act
            .min(self.courses.len().saturating_sub(1));
        &self.courses[act]
    }
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
    /// One course per act; they are all on screen at once.
    pub courses: Vec<CourseSnapshot>,
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
    /// Height of the surface under her: the tent floor, or a stage top when
    /// she is standing on one. Eased, so a stage edge is a step and not a jump.
    ground: f32,
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
    /// Solids she must not walk into, in world coordinates as x, z, radius.
    ///
    /// The body owns its own collision, because the body is what moves. A
    /// resolution pass running after the movement has already happened cannot
    /// help: the steering recomputes the velocity from scratch every tick and
    /// throws away anything an outside pass wrote to it, so a character walks
    /// into a wall at full speed and is then pushed back out of it forever.
    ///
    /// The world fills this in; the body resolves against it.
    solids: Vec<[f32; 3]>,
    /// Which of the four gaits the character currently walks with.
    gait: usize,
    /// The largest circle she can mill around on this stage without touching
    /// its scenery. Set from the act, so a cramped stage cramps her.
    orbit_room: f32,

    // ---- the course ----
    //
    // She has a job now. The act's stage used to be the whole of her purpose and
    // the "adventure" was a point drifting around it, so the most she ever did was
    // walk to the middle of a disc. The course gives her somewhere to be and a
    // reason to walk there, and the three fields below are how she finds it.
    /// Where she is heading, in arena coordinates. The runtime sets this each
    /// tick from what she can smell and what the others have left behind.
    nav: [f32; 2],
    /// How much of her attention the course takes from milling about, 0..1.
    /// At zero she orbits her act exactly as she always did, which is what the
    /// acts that are not running a course get.
    nav_weight: f32,
    /// Whether `nav` is a place she has remembered, rather than a direction she is
    /// feeling her way along.
    ///
    /// The two are not mixed. A remembered place is one cell away and she walks at
    /// it; a direction she is feeling for is the whole wall-following rule, and
    /// applying both at once has her hugging a wall while trying to reach the room
    /// behind her, which is a reliable way to reach no room at all.
    nav_is_place: bool,
    /// Whether there are walls here to feel along.
    ///
    /// On open ground the wall-following rule has nothing to work with: every
    /// direction reads clear, so it commits to the first one and walks it, which is
    /// not where the food is. With no maze there is nothing to search, so she is
    /// pointed at it and left to walk.
    nav_walls: bool,
    /// A quarter turn to add to her heading, and how long it lasts.
    ///
    /// This is the whole of her wall handling. Without it she walks into a dead
    /// end and stays there, because the food smells the same from every side of
    /// the wall between them. With it she comes back along the wall and tries
    /// the next opening, which is what an insect does and what makes the round
    /// look like a search rather than a queue.
    wall_turn: f32,
    wall_timer: f32,
    /// Whether she has reached the food this round.
    fed: bool,
    /// The deepest of the maze's foods she has reached, counting from the entrance.
    ///
    /// Zero is the nearest one. A maze of a hundred rooms is not finished in a
    /// round, so the round's question is not "did she arrive" but "how far in did
    /// she get", and this is the number that answers it.
    ate_feed: usize,
    /// How many of the maze's foods she has had.
    ate_count: u32,
    /// The stage she is on, as centre x, centre z and radius.
    ///
    /// The rim is a real boundary and it belongs with the collision rather than
    /// with a clamp outside it. Without it, being pushed out of a maze wall near
    /// the edge of the stage pushes her off the stage, and a character standing
    /// on the tent floor is a different character from one standing on a disc.
    stage: [f32; 3],
    /// How long she has been going nowhere, in seconds.
    ///
    /// Measured from her own displacement, not from whether she wants to move: a
    /// character can want a door very much and still not get through it, and only
    /// the displacement says which of the two is happening.
    stuck: f32,
    /// Which way she last tried to get herself out of a wedge, so that the next
    /// try is a different one.
    stuck_way: f32,
    /// The direction she is walking, kept across ticks.
    ///
    /// The course has to be searched rather than solved, and searching needs a
    /// sense of which way she is facing. Taking it from her velocity alone is not
    /// enough: she is eased toward her heading, so velocity is near zero for a
    /// moment after every turn and the rule below would read that as "no way
    /// open at all".
    facing: [f32; 2],
    /// Where she has been this round, in arena coordinates, newest last.
    ///
    /// This is the only thing she teaches the others. A character that has eaten
    /// leaves a path, and a character that has not will follow it, which is
    /// learning from another one without anything being transferred but a
    /// direction.
    trail: std::collections::VecDeque<[f32; 2]>,
    /// What she remembers about the course, or nothing if she is not on one.
    search: Option<Search>,
}

/// What a character remembers about where she has been on this course.
///
/// This is the difference between a character that searches a maze and one that
/// is stuck in it. Feeling her way along the walls works until she meets a dead
/// end, and then it works her in circles: the way back smells exactly like the way
/// on, and nothing in the walls tells the two apart. So she writes down the road
/// she took, and writes a place off once every way out of it has been tried.
///
/// The passages themselves are *not* given to her. What connects to what is
/// learned by walking, one step at a time, from the cell she was standing in to
/// the cell she reached. She is given the names of the places and nothing about
/// what lies between them, which is very much less than a map.
#[derive(Debug, Clone)]
struct Search {
    /// The road she took to get here, entrance first. The last entry is where she
    /// is standing.
    stack: Vec<usize>,
    /// Set for a place she has stood in and walked out of.
    ///
    /// Not the same as "finished". A place can be stood in, left, and still have
    /// somewhere new, and the difference is the whole search: without this she
    /// treats the doorway she came through as somewhere to try, walks back into
    /// the room she just left, and shuttles between two cells for the whole round.
    seen: Vec<bool>,
    /// Set for a place whose every way out leads to a finished branch.
    ///
    /// Deliberately not the same as being new to it. A place she has not left yet
    /// is unexplored, and writing that off is what makes a character mark every
    /// cell she has not yet walked out of as a dead end, learn nothing, and stand
    /// still.
    dead: Vec<bool>,
    /// The passages she has found, both ways round, by walking them.
    links: Vec<Vec<usize>>,
    /// The cell she has set off for, which she does not change her mind about
    /// until she is standing in it.
    ///
    /// Without this she changes her mind sixty times a second in a doorway. The
    /// boundary between two cells runs through the middle of the opening between
    /// them, so standing in the opening names one cell and then the other and then
    /// the first, and a character deciding afresh each tick turns round in the
    /// threshold and never goes through. Deciding once and then walking it is what
    /// a decision is.
    committing: Option<usize>,
}

impl Search {
    /// She is handed the passages and nothing else. The road, what has been tried
    /// and what she is on her way to are all hers.
    fn new(passages: &[Vec<usize>]) -> Self {
        Self {
            stack: Vec::with_capacity(8),
            seen: vec![false; passages.len()],
            dead: vec![false; passages.len()],
            links: passages.to_vec(),
            committing: None,
        }
    }

    /// Note that she has got to a cell, keeping the road she took to it.
    ///
    /// Retracing trims the road back to the place she has just come from, so the
    /// road is always the part of the maze she has not finished. A place she has
    /// never stood in is simply added.
    fn arrive(&mut self, cell: usize) {
        if self.stack.last() == Some(&cell) {
            return;
        }
        if let Some(at) = self.stack.iter().rposition(|c| *c == cell) {
            self.stack.truncate(at + 1);
        } else {
            self.stack.push(cell);
        }
    }

    /// The cell to make for next, or nothing while she is still getting her
    /// bearings or has arrived.
    ///
    /// Nowhere she has never stood, if there is one. Failing that, the place she
    /// came from, so she walks back to it; arriving there trims the road and the
    /// next call carries on from there. On a carved maze that is a tree, so this
    /// reaches every part of it, and it stops at the food.
    ///
    /// The retreat is a place to walk to, not a place she has been moved to. It
    /// used to pop the road here instead, which is a searcher deciding that she
    /// is somewhere she is not: she stood in one corner of the maze while her own
    /// record said she was three rooms away, so the next thing it wanted her to do
    /// was walk through a wall, and she did not move for the rest of the round.
    fn next(&mut self, food: usize) -> Option<usize> {
        // A decision already made stands until she has physically got there, or
        // until the place she was going turns out to be finished with. Deciding
        // from the road alone does not work: the boundary between two cells runs
        // through the middle of the doorway between them, so she is named in one
        // cell and then the other and then the first, and a character that
        // re-decides on every tick turns round in the threshold and never goes
        // through.
        if let Some(going) = self.committing
            && !self.dead[going]
        {
            return Some(going);
        }
        self.committing = None;
        let here = *self.stack.last()?;
        if here == food {
            return None;
        }
        // Somewhere she has never stood. Tried on its own: the doorway behind her
        // is a way out but not a way *on*, and offering it as one is what turns a
        // search into a shuffle between two cells.
        if let Some(next) = self
            .links
            .get(here)
            .and_then(|out| out.iter().copied().find(|c| !self.seen[*c]))
        {
            self.seen[here] = true;
            self.committing = Some(next);
            return Some(next);
        }
        // Nothing new from here, so this branch is finished. She walks back to
        // where she came from; the road is trimmed when she gets there.
        self.seen[here] = true;
        self.dead[here] = true;
        let at = self.stack.iter().rposition(|c| *c != here)?;
        Some(self.stack[at])
    }
}

/// How far apart two trail points have to be before the next one is kept.
///
/// The trail is a breadcrumb for the renderer and a lure for the other
/// characters, and neither needs a point every centimetre. Thin enough to be
/// cheap, thick enough to read as a path.
pub const TRAIL_STEP: f32 = 0.09;

/// How long a character keeps to a wall after bumping into one.
pub const WALL_COMMITMENT: f32 = 0.55;

/// How deep into a wall she has to be before it counts as being stuck.
///
/// A graze is not a jam. In a corridor she touches both sides continuously, and
/// treating that as being stuck turns her quarter every few frames, which sends
/// her round in circles instead of through the maze.
pub const JAMMED: f32 = 0.055;

/// How close to the food counts as reaching it.
///
/// A little over a body width, so she has to come to it rather than brush past.
pub const FOOD_REACH: f32 = BODY_RADIUS + 0.09;

/// How long she may go nowhere before she tries a different angle, in seconds.
pub const STUCK_AFTER: f32 = 0.6;

/// Less ground than this in one tick counts as going nowhere.
///
/// A twentieth of her body width. She walks about a hundredth of a unit a tick when
/// she is moving, so this only catches her when she is not.
pub const STUCK_STEP: f32 = 0.0015;

/// How far off her intended line she tries when wedged, in radians.
///
/// Enough to miss the corner that is catching her and little enough that she is
/// still going roughly where she meant to.
pub const DEFLECT: f32 = 0.85;

/// How far ahead she feels for a wall, in world units.
///
/// One stride, and it has to be shorter than the room she has to move sideways,
/// which is what lets her turn. A longer probe asks "could I walk half a metre
/// that way", and in a passage the honest answer sideways is no, so she finds no
/// way open at all and stands still. A stride asks "can I take the next step that
/// way", which is the decision she is actually making.
///
/// It does not need to be longer than her keep-out from a wall, because the sense
/// is measured against the wall *plus a body width*: a wall across the passage is
/// felt a whole body before she is against it, which is what lets her turn in time
/// instead of walking the length of a corridor and overshooting the junction.
pub const PROBE: f32 = 0.15;

/// How much slack the sense allows beyond her own width.
pub const PROBE_MARGIN: f32 = 0.02;

/// The longest a gait trial may move her in one go, in world units.
///
/// Short enough that she cannot clear a corridor in a single step. The collision
/// resolves what she is standing inside, so the move has to be small enough that
/// she is never more than a step past a wall before it is checked.
pub const ADVANCE_SLICE: f32 = 0.12;

/// How many points a character's trail keeps.
///
/// Long enough to show the way back along a stage, short enough that it stays a
/// handful of points per character in the snapshot.
pub const TRAIL_MAX: usize = 48;

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
            // Height of the surface she is standing on: the tent floor, or a
            // stage top when she is on one.
            ground: 0.0,
            // Replaced the moment she is put on an act, which is before the
            // first step, so it is never actually used as empty.
            solids: Vec::new(),
            orbit_room: 0.0,
            nav: [0.0, 0.0],
            nav_weight: 0.0,
            nav_is_place: false,
            nav_walls: false,
            // Every character turns the same way round a wall, which is what
            // makes a troupe traverse a maze instead of milling in it. The sign
            // comes from her id so two characters do not cancel out.
            wall_turn: if id.is_multiple_of(2) { 1.0 } else { -1.0 },
            wall_timer: 0.0,
            fed: false,
            ate_feed: usize::MAX,
            ate_count: 0,
            stage: [0.0, 0.0, 0.0],
            stuck: 0.0,
            stuck_way: 1.0,
            facing: [0.0, 0.0],
            trail: std::collections::VecDeque::new(),
            search: None,
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
    /// Walk out of anything she has ended up inside.
    ///
    /// A pass over the solids, twice, because pushing her out of one can push
    /// her into another where they nearly touch. The correction is purely
    /// positional: where she ends up is decided by the steering, and this only
    /// refuses to leave her standing inside a wall.
    ///
    /// Returns whether she was actually inside something, which is the steering's
    /// cue to go round it instead of into it. Without that cue a character walks
    /// into a dead end and stays there: the food smells the same from both sides
    /// of the wall, so nothing in the attraction tells her to turn.
    fn slide_out_of_solids(&mut self) -> f32 {
        let mut worst = 0.0f32;
        for _ in 0..3 {
            // Every overlap is collected and the total is applied in one go, rather
            // than resolving one wall at a time.
            //
            // Where two walls meet, resolving them in turn is a pendulum: pushed off
            // one, straight into the other, back again, and she is wedged in the
            // corner until something else walks into her and shakes her loose. A
            // character on her own has nobody to do that, so she stood in the
            // first corner of every maze for the whole round. Added together the
            // two pushes point out along the diagonal, which is the way out.
            let mut push_x = 0.0f32;
            let mut push_y = 0.0f32;
            for solid in &self.solids {
                let (sx, sz, r) = (solid[0], solid[1], solid[2]);
                if r <= 0.0 {
                    continue;
                }
                let wanted = r + BODY_RADIUS;
                let nx = self.position[0] - sx;
                let ny = self.position[1] - sz;
                let nd = (nx * nx + ny * ny).sqrt();
                if (nd >= wanted) || (nd < 1e-4) {
                    continue;
                }
                let depth = wanted - nd;
                push_x += (nx / nd) * depth;
                push_y += (ny / nd) * depth;
                worst = worst.max(depth);
            }
            self.position[0] += push_x;
            self.position[1] += push_y;
            // And she stops pushing. The steering is recomputed from scratch every
            // tick, so the velocity still carries the whole of the shove that got
            // her into this wall, and she walks into it again at the same speed
            // until she is stopped dead by it. Taking the component of her
            // velocity that goes into the wall off is what makes her slide along
            // the wall and round it, which is the difference between a character
            // negotiating a maze and a character leaning on one.
            let push = (push_x * push_x + push_y * push_y).sqrt();
            if push > 1e-6 {
                let (ux, uy) = (push_x / push, push_y / push);
                let into = self.velocity[0] * ux + self.velocity[1] * uy;
                if into < 0.0 {
                    self.velocity[0] -= ux * into;
                    self.velocity[1] -= uy * into;
                }
            }
            // Then the rim. A wall near the edge of the stage pushes her out to
            // `wall + body`, which for a wall at the edge is past the stage, so
            // without this she steps off the disc and is standing on the floor.
            let (cx, cz, radius) = (self.stage[0], self.stage[1], self.stage[2]);
            if radius > 0.0 {
                let limit = radius - BODY_RADIUS;
                let dx = self.position[0] - cx;
                let dz = self.position[1] - cz;
                let d = (dx * dx + dz * dz).sqrt();
                if d > limit && d > 1e-4 {
                    self.position[0] = cx + (dx / d) * limit;
                    self.position[1] = cz + (dz / d) * limit;
                    worst = worst.max(d - limit);
                }
            }
            if worst <= 0.0 {
                break;
            }
        }
        worst
    }

    /// Is the way clear a short way off in this direction?
    ///
    /// This is her only sense of the walls. There is no map and no plan: she asks
    /// whether she herself fits a stride that way, in each of four directions, and
    /// that is enough to walk a maze.
    ///
    /// The test is against the wall *plus a body width*, not against the wall. A
    /// point fits through gaps a person does not, so asking "is that spot clear"
    /// tells her a corridor is open when she is too wide for it, and she walks into
    /// a wall she had already decided was not there.
    fn way_open(&self, dx: f32, dy: f32) -> bool {
        // The stage rim counts as a wall, or she would happily walk off the disc
        // following it.
        let (cx, cz, radius) = (self.stage[0], self.stage[1], self.stage[2]);
        let reach = BODY_RADIUS + PROBE_MARGIN;
        let clear = |px: f32, py: f32| {
            if radius > 0.0 && (px - cx).hypot(py - cz) > radius - BODY_RADIUS {
                return false;
            }
            !self.solids.iter().any(|solid| {
                solid[2] > 0.0 && (px - solid[0]).hypot(py - solid[1]) < solid[2] + reach
            })
        };
        // One stride, and it is short on purpose. A long probe asks "could I walk
        // half a metre that way", and in a passage the honest answer sideways is
        // no — which is right, and useless, because turning *is* a sideways
        // moment. She then finds no way open at all and stands still. A stride
        // asks "can I take the next step that way", which is what she is actually
        // deciding, and the body width it is measured against means a wall across
        // the passage is felt a whole body before she is against it.
        clear(self.position[0] + dx * PROBE, self.position[1] + dy * PROBE)
    }

    /// Which way to go next, searching.
    ///
    /// Wall following, which is how an insect does it and what actually works
    /// here. The smell of the food is her initial heading and nothing more: on
    /// its own it walks her into the first dead end and she stays there, because
    /// the food smells the same from both sides of the wall between them.
    ///
    /// The priority is fixed: turn towards the wall you are keeping, then go on,
    /// then turn the other way, then turn about. A carved maze is a tree, so a
    /// character that keeps to a wall this way comes to every part of it. Letting
    /// the smell choose between the open ways instead looks more purposeful and
    /// does not work at all: she dithers on the spot, because the smell points back
    /// the way she came.
    ///
    /// Which hand she keeps is hers, so a troupe does not all file through the
    /// same corridor in the same order.
    /// Which way to go next, searching.
    ///
    /// Wall following, which is how an insect does it and what actually works
    /// here. The smell of the food is her initial heading and nothing more: on
    /// its own it walks her into the first dead end and she stays there, because
    /// the food smells the same from both sides of the wall between them.
    ///
    /// The order is the whole algorithm: carry on if the way is clear, otherwise
    /// turn towards the wall she is keeping, as far as she has to. A carved maze
    /// is a tree, so a character that keeps to a wall this way comes to every part
    /// of it. Which hand she keeps is hers, so a troupe does not all file through
    /// the same corridor in the same order.
    ///
    /// Eight directions, not four. With only four she can be boxed in: facing a
    /// diagonal, all four of straight, left, right and about are walls while the
    /// way on is open, and there is no move in the set for her to make. She then
    /// turns about every tick, which is what a character in a dead end used to do
    /// for a whole round.
    fn search_heading(&mut self, to_goal: [f32; 2]) -> [f32; 2] {
        // Her facing is whatever the rule last chose, and nothing else.
        //
        // It is tempting to read it off her velocity, and that is what this did at
        // first, and it does not work: being pushed off a wall puts a sideways
        // component into the velocity, the rule reads that as her heading, turns
        // her, the turn pushes her into the other wall, and she spends the round
        // vibrating between two directions and going nowhere. The heading is a
        // decision, and a decision does not come from what the body did last tick.
        //
        // Before she has chosen anything she heads for the food, which is the one
        // time the smell is allowed to decide.
        if self.facing[0].abs() + self.facing[1].abs() < 1e-4 {
            let gx = to_goal[0] - self.position[0];
            let gy = to_goal[1] - self.position[1];
            let d = gx.hypot(gy);
            if d < 1e-4 {
                return [0.0, 0.0];
            }
            self.facing = [gx / d, gy / d];
        }
        let (fx, fy) = (self.facing[0], self.facing[1]);
        let hand = if self.wall_turn >= 0.0 {
            1.0f32
        } else {
            -1.0f32
        };
        // The order is the rule, and the order matters more than the set.
        //
        // Squarely towards the wall she keeps comes *before* straight on.
        // Preferring to go straight lets her leave the wall at every junction, and
        // a character that has lost her wall is not following anything: she crosses
        // the middle of the maze, comes back, and crosses it again, which is what
        // she did for as long as this preferred straight. With the wall kept
        // first, a carved maze being a tree means she comes to every part of it.
        //
        // Eight headings, 45 degrees apart, because with four she can be boxed in:
        // facing a diagonal, straight, both sides and about are all walls while the
        // way on is open, and there is no move in the set for her to make.
        let steps = [2.0f32, 1.0, 0.0, -1.0, -2.0, -3.0, 4.0, 3.0];
        // While she is wedged she starts further round, skipping the turns she has
        // just failed to make. A corner her feeling calls open but her shoulders
        // do not fit through is the one thing the feeling cannot see, and without
        // this she commits to it, is pushed back, commits again, and works the
        // corner for the rest of the round instead of going round it.
        let skip = usize::from(self.wall_timer > 0.0) * 2;
        self.wall_timer = 0.0;
        for (index, step) in steps.into_iter().enumerate() {
            if index < skip {
                continue;
            }
            let angle = hand * step * std::f32::consts::FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            let candidate = [fx * cos - fy * sin, fx * sin + fy * cos];
            if self.way_open(candidate[0], candidate[1]) {
                self.facing = candidate;
                return candidate;
            }
        }
        // Boxed in on every side, which the disc-cut maze should not allow. Hold
        // the heading rather than spinning.
        [fx, fy]
    }

    /// Move her forward along her heading, for a gait trial.
    ///
    /// Moved in slices rather than in one jump. The distance is measured so the
    /// brain can be scored on how far it got, but a single jump of half a unit
    /// clears a wall in one go: the collision only pushes her out of whatever she
    /// landed inside, and by then she is on the far side, where nothing pushes
    /// her back. A maze walked that way is a maze she is not in.
    fn advance(&mut self, steps: u32, dt: f32) {
        let speed = 0.6 + 0.5 * self.gait_cadence;
        let distance = speed * dt * steps as f32;
        let heading = if self.velocity[0].abs() + self.velocity[1].abs() > 1e-4 {
            (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt()
        } else {
            1.0
        };
        let (ux, uy) = (self.velocity[0] / heading, self.velocity[1] / heading);
        let slices = ((distance / ADVANCE_SLICE).ceil() as usize).max(1);
        let slice = distance / slices as f32;
        for _ in 0..slices {
            self.position[0] += ux * slice;
            self.position[1] += uy * slice;
            self.slide_out_of_solids();
        }
        for (axis, limit) in [(0, WORLD_LIMIT), (1, WORLD_LIMIT)] {
            self.position[axis] = self.position[axis].clamp(-limit, limit);
        }
    }

    fn step(&mut self, time: f32, dt: f32, speed: f32, decay: f32) {
        let from = self.position;
        let phase = time * 0.9 + self.id as f32 * 1.31;
        self.light = (0.5 + 0.35 * (time * 0.7).sin()).clamp(0.0, 1.0);
        self.odor = (0.5 + 0.3 * (time * 0.43 + self.id as f32 * 0.17).sin()).clamp(0.0, 1.0);
        self.temperature = (0.5 + 0.15 * (time * 0.2).sin()).clamp(0.0, 1.0);
        let boundary = ((self.position[0].abs() - WORLD_LIMIT).max(0.0)
            + (self.position[1].abs() - WORLD_LIMIT).max(0.0))
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
        //
        // The orbit is clamped to what the stage can actually offer. Left
        // unclamped it ran straight through the scenery, and a character
        // pressed against a wall with the stage still pulling her in grinds
        // there instead of walking around it.
        let orbit_r = (0.55 + 0.18 * self.id as f32).min(self.orbit_room);
        let orbit_a = time * 0.5 + self.id as f32 * 2.1;
        let goal_x = self.anchor[0] + orbit_r * orbit_a.cos();
        let goal_y = self.anchor[1] + orbit_r * orbit_a.sin();
        // On a course the goal is somewhere she has to find, not a circle she
        // drifts around. Blending the two by `nav_weight` keeps one steering
        // path for both, so the acts that are not running a course behave exactly
        // as they did.
        let seek_x = goal_x * (1.0 - self.nav_weight) + self.nav[0] * self.nav_weight;
        let seek_y = goal_y * (1.0 - self.nav_weight) + self.nav[1] * self.nav_weight;
        // The wall commitment turns her heading a quarter turn while it lasts.
        // Applied to the direction she is trying to go, not to the world, so it
        // is "carry on past this wall" rather than "go that way now".
        let to_seek_x = seek_x - self.position[0];
        let to_seek_y = seek_y - self.position[1];
        let (mut aimed_x, mut aimed_y) = (to_seek_x, to_seek_y);
        // On a course with nothing remembered she feels her way along the walls.
        // With a place remembered she walks straight at it, because it is the next
        // room and the way into it is a doorway she can see.
        //
        // These are exclusive, and the wall commitment is not allowed to leak into
        // the second case. It did, through an `else if` that only guarded the
        // wall-following branch: she was being nudged by a wall, the commitment
        // turned her aim a quarter turn, she walked into the doorway sideways and
        // got nudged again, and she stood in the threshold of the first room of
        // every maze for the whole round with a remembered destination and no way
        // of walking to it.
        if self.nav_weight > 0.0 {
            if !self.nav_is_place && self.nav_walls {
                let chosen = self.search_heading([to_seek_x, to_seek_y]);
                aimed_x = chosen[0];
                aimed_y = chosen[1];
            }
            // Wedged. Try the same way from a different angle, alternating which
            // way that is.
            //
            // This is what anything with legs does when it is stuck, and it is the
            // only thing that saves a character on her own. Two walls meeting at a
            // corner cannot be resolved by pushing, because being pushed off one
            // puts her into the other; a companion walks into her and shakes her
            // loose, which is why a troupe always got through and a lone character
            // stood in the first doorway for the whole round.
            if self.stuck > STUCK_AFTER {
                let (sin, cos) = (self.stuck_way * DEFLECT).sin_cos();
                (aimed_x, aimed_y) = (
                    to_seek_x * cos - to_seek_y * sin,
                    to_seek_x * sin + to_seek_y * cos,
                );
                self.stuck_way = -self.stuck_way;
            }
        } else if self.wall_timer > 0.0 {
            let (sin, cos) = (self.wall_turn * std::f32::consts::FRAC_PI_2).sin_cos();
            (aimed_x, aimed_y) = (
                to_seek_x * cos - to_seek_y * sin,
                to_seek_x * sin + to_seek_y * cos,
            );
            self.wall_timer = (self.wall_timer - dt).max(0.0);
        }
        // The wander is what makes a milling character look alive, and it is
        // exactly what stops a searching one from ever arriving. It fades out as
        // the course takes her attention rather than being a separate code path,
        // so there is still only one steering rule to reason about.
        let idle = 1.0 - self.nav_weight;
        let desired_x = aimed_x * 0.9
            + (self.odor - 0.5) * 0.35 * idle
            + (time * 0.35 + self.id as f32).sin() * 0.2 * idle;
        let desired_y = aimed_y * 0.9
            + (self.light - 0.5) * 0.25 * idle
            + (time * 0.27 + self.id as f32 * 1.7).cos() * 0.15 * idle;
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
        // Whether she is actually getting anywhere. Measured from the ground she
        // covered, because wanting to go somewhere and being able to go there are
        // two different things and only one of them is progress.
        let covered = (self.position[0] - from[0]).hypot(self.position[1] - from[1]);
        self.stuck = if covered < STUCK_STEP {
            (self.stuck + dt).min(STUCK_AFTER * 3.0)
        } else {
            0.0
        };
        // She has moved, so she may now be inside something. Resolving here,
        // immediately after the move, is the only place it works: a pass that
        // runs later finds a position that has already passed through a wall.
        //
        // The depth of the push decides whether it counts as being stuck. Grazing
        // a wall is normal in a corridor and must not turn her, or she spends the
        // whole round quarter-turned and walks in circles.
        if self.slide_out_of_solids() > JAMMED {
            // Genuinely wedged. Commit to going round it rather than into it,
            // which is the difference between searching a maze and being stuck in
            // the first dead end.
            self.wall_timer = WALL_COMMITMENT;
        }
        // Remember where she got to, but only every so often: a point per
        // centimetre is a very long trail and says nothing more than one per
        // hand's width.
        //
        // The first point is written as soon as there is a trail to write into,
        // and that is not a detail. Comparing against "the last point, or here if
        // there are none" compares her against where she already is, which is
        // always zero steps, so the trail never gets its first point and never
        // gets any at all.
        let last = self.trail.back().copied();
        let moved_on = match last {
            Some(point) => {
                (self.position[0] - point[0]).hypot(self.position[1] - point[1]) > TRAIL_STEP
            }
            None => true,
        };
        if moved_on {
            self.trail.push_back([self.position[0], self.position[1]]);
            while self.trail.len() > TRAIL_MAX {
                self.trail.pop_front();
            }
        }

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
        for (axis, limit) in [(0, WORLD_LIMIT), (1, WORLD_LIMIT)] {
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

/// How many cells across a course's grid.
///
/// Six is as many as the passages will take: a character is `BODY_RADIUS` wide
/// and a corridor has to be wider than that, so the generator drops a cell from
/// the grid rather than narrowing the walls. A smaller maze is walkable and a
/// larger one is not.
pub const COURSE_CELLS: usize = 6;

/// How long a round lasts before the course is rebuilt under them.
///
/// Measured, not guessed. Searching a maze takes a character somewhere between
/// nine seconds on an easy one and a couple of minutes on a hard one, and the
/// median first arrival over a spread of mazes is about half a minute. A round
/// has to be long enough that the interesting thing is usually the first one to
/// arrive and the others following her, rather than the clock.
pub const ROUND_LENGTH: f32 = 62.0;

/// How long the round is left standing once everyone has eaten, in seconds.
///
/// Long enough to see. Without it the last arrival ends the round on the tick she
/// touches the food, the maze is rebuilt under their feet, and they are back at
/// the entrance before a frame is drawn: nobody sees the result and the panel
/// never gets to say who won.
pub const FEED_PAUSE: f32 = 3.5;

// ===========================================================================
// The five acts
//
// Each act is a place, a stimulus, and one thing the fly has to learn. The
// lesson is a cue/action pair, so the fly's own associative weights decide
// whether it solves the act. Nothing is scripted as a success animation: the
// mastery bar reflects the real weight the C core accumulated.
// ===========================================================================

/// A circus act: where it is, what it looks like, and what it teaches.
/// Something solid on a stage that a character cannot walk through.
///
/// Circles rather than boxes, because a circle is all the runtime needs to push
/// a character out and it cannot be entered from an impossible angle. Walls are
/// approximated by a circle on their midpoint, which is close enough for
/// something the width of a person.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Collider {
    /// Centre in act-local coordinates, so it moves with the stage.
    pub x: f32,
    pub z: f32,
    pub radius: f32,
}

/// A circle in act-local space, computed once so the act table stays readable.
const fn at(x: f32, z: f32, radius: f32) -> Collider {
    Collider { x, z, radius }
}

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
    /// Radius of the walkable stage top.
    pub stage_radius: f32,
    /// How high the stage stands above the tent floor. A character standing on
    /// it is on top of it, not inside it, which is why this is not decoration.
    pub stage_height: f32,
    /// Solids on the stage, in act-local coordinates.
    pub colliders: &'static [Collider],
    /// How many cells across this district's maze should be.
    ///
    /// Zero means open ground with no maze at all, which is the hill: a fly wakes
    /// up there, and the one thing that district must not have is somewhere to be
    /// lost. Everything else is a maze to be got through.
    pub maze_cells: usize,
}

/// The four corner posts every district shares, on a ring at the diagonals.
/// Written out rather than computed, because `cos` is not const and four numbers
/// beat a helper that cannot run.
static CORNER_POSTS: [Collider; 4] = [
    at(3.30, 3.30, 0.10),
    at(-3.30, 3.30, 0.10),
    at(-3.30, -3.30, 0.10),
    at(3.30, -3.30, 0.10),
];

/// The tree on the hill: a trunk she can bump into, and the one landmark a fly
/// learns to find her way back to.
///
/// It is the only solid on the hill besides the posts, because the hill has no
/// maze and nothing else to walk into. A newborn fly with one thing in the world
/// to learn has somewhere to be lost and somewhere to get back to.
static HILL_TREE: [Collider; 1] = [at(1.9, -1.4, 0.62)];

/// How many solids an act can hold. The four posts every stage shares, plus
/// whatever the act puts on top of them.
const MAX_SOLIDS: usize = 8;

/// Pad with a collider of zero radius, parked far outside every stage, so an
/// unused slot can never push a character anywhere.
const fn padding() -> Collider {
    at(1000.0, 1000.0, 0.0)
}

/// Put a stage's own scenery on top of the posts it shares with the others.
const fn solids_with(extra: &[Collider]) -> [Collider; MAX_SOLIDS] {
    let mut out = [padding(); MAX_SOLIDS];
    let mut n = 0;
    while n < CORNER_POSTS.len() {
        out[n] = CORNER_POSTS[n];
        n += 1;
    }
    let mut i = 0;
    while i < extra.len() && n < MAX_SOLIDS {
        out[n] = extra[i];
        n += 1;
        i += 1;
    }
    out
}

/// Scenery for the districts that have any.
///
/// The hill has the tree. The rest are mazes, and their walls come from the course
/// and are rebuilt every round, so there is nothing fixed to list here.
static HILL_SOLIDS: [Collider; MAX_SOLIDS] = solids_with(&HILL_TREE);
static OPEN_SOLIDS: [Collider; MAX_SOLIDS] = solids_with(&[]);

/// Height of every district's ground above the world floor.
const STAGE_HEIGHT: f32 = 0.17;

/// How big a district is.
///
/// It was 1.7, and that is what held the maze at four cells across. The cell size
/// is set by how much room a passage has to give a character, not by how big the
/// ground is, so a bigger maze needs a bigger place, not tighter walls. At 7 the
/// grid comes out around thirteen cells across instead of four: a maze of about a
/// hundred and forty rooms rather than twelve.
const STAGE_RADIUS: f32 = 5.3;

/// How far apart the districts sit.
///
/// Comfortably more than two radii, so one district's outer wall can never reach
/// into the next one's ground.
pub const DISTRICT_GAP: f32 = 13.0;

/// The edge of the world, past which a character is not allowed to be.
///
/// The districts reach `DISTRICT_GAP` plus a radius from the middle, so this has to
/// clear that. It used to be 9.2 for districts 6 apart.
pub const WORLD_LIMIT: f32 = 22.0;

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
/// Stable ASCII keys, index-for-index with `EMOTION_NAMES` and the `T_EMO_*`
/// order in the C core.
///
/// These exist so the rig can bind to a feeling without matching on a Russian
/// label. A label is for people and gets reworded; a key is for code and does
/// not. The order here is load-bearing: it must stay aligned with the header,
/// which a test enforces by asking the C core for its own names rather than by
/// comparing against a second copy of the list.
pub const EMOTION_KEYS: [&str; 26] = [
    "pain",
    "fear",
    "joy",
    "sadness",
    "anger",
    "disgust",
    "surprise",
    "curiosity",
    "lust",
    "craving",
    "contentment",
    "dread",
    "confusion",
    "pride",
    "gratitude",
    "relief",
    "disappointment",
    "hope",
    "jealousy",
    "shyness",
    "affection",
    "boredom",
    "excitement",
    "compassion",
    "trust",
    "longing",
];

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
    // Where a fly wakes up. Bare ground, one tree, and nothing else to do but
    // get up.
    Act {
        id: "hill",
        name: "Холм",
        subtitle: "Дерево, склон, тишина",
        lesson: "Первый шаг стоит дороже всех остальных",
        origin: [0.0, 0.0],
        color: "#8ce06a",
        cue: cue::LIGHT,
        solution: action::FORWARD,
        distractor: action::REST,
        ambience: action::REST,
        stage_radius: STAGE_RADIUS,
        stage_height: STAGE_HEIGHT,
        colliders: &HILL_SOLIDS,
        maze_cells: 0,
    },
    // A village: the same maze as before, but a big one, between houses.
    Act {
        id: "village",
        name: "Деревня",
        subtitle: "Дома, тропы, заборы",
        lesson: "Запах еды ведёт через избы",
        origin: [DISTRICT_GAP, 0.0],
        color: "#ffb86b",
        cue: cue::ODOR_FRUIT,
        solution: action::EAT,
        distractor: action::DANCE,
        ambience: action::FORWARD,
        stage_radius: STAGE_RADIUS,
        stage_height: STAGE_HEIGHT,
        colliders: &OPEN_SOLIDS,
        maze_cells: 11,
    },
    // The city, and the biggest maze of all. A fly that can find her way out of
    // this has learnt something.
    Act {
        id: "city",
        name: "Город",
        subtitle: "Башни, улицы, фонари",
        lesson: "В городе запах тонет в шуме",
        origin: [0.0, DISTRICT_GAP + 2.0],
        color: "#54d7e8",
        cue: cue::SOUND,
        solution: action::TURN_R,
        distractor: action::TURN_L,
        ambience: action::FORWARD,
        stage_radius: STAGE_RADIUS,
        stage_height: STAGE_HEIGHT,
        colliders: &OPEN_SOLIDS,
        maze_cells: 12,
    },
    // Dark. The lesson is standing still, which is the one thing a character that
    // has just learnt to walk everywhere is worst at.
    Act {
        id: "forest",
        name: "Лес",
        subtitle: "Смола, тень, чужой запах",
        lesson: "В темноте нужно замереть",
        origin: [-DISTRICT_GAP, 0.0],
        color: "#a98bff",
        cue: cue::DARK,
        solution: action::REST,
        distractor: action::DANCE,
        ambience: action::REST,
        stage_radius: STAGE_RADIUS,
        stage_height: STAGE_HEIGHT,
        colliders: &OPEN_SOLIDS,
        maze_cells: 10,
    },
    // Open ground with nothing to hide behind.
    Act {
        id: "ruins",
        name: "Руины",
        subtitle: "Обломки, ветер, длинный свет",
        lesson: "На открытом месте идти прямо",
        origin: [0.0, -(DISTRICT_GAP + 2.0)],
        color: "#ff5c7a",
        cue: cue::VIBRATION,
        solution: action::FORWARD,
        distractor: action::REST,
        ambience: action::FLAP,
        stage_radius: STAGE_RADIUS,
        stage_height: STAGE_HEIGHT,
        colliders: &OPEN_SOLIDS,
        maze_cells: 12,
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

/// The modulator's terms, read back out of the fly.
///
/// The gate is the third factor of the three-factor rule, and in a trial it is
/// the only one of the three that can be missing: the other two are built into
/// the trial. As a bare percentage it says nothing about why a character is not
/// learning, which is the one thing a reader wants to know. These are the same
/// terms, in the same order and with the same coefficients as `TLearningGate`
/// in the core, so the parts add up to the gate. That is also a second copy of
/// the formula, and a second copy drifts, so `gate_terms_sum_to_the_gate` asks
/// the core for the total and adds these up: a reweighting on either side
/// fails there rather than quietly making the breakdown a lie.
fn gate_terms(fly: &tfly::Fly) -> Vec<GateTerm> {
    let dopamine = fly.hormone_level(tfly::hormone::DOPAMINE);
    let octopamine = fly.hormone_level(tfly::hormone::OCTOPAMINE);
    let serotonin = fly.hormone_level(tfly::hormone::SEROTONIN);
    let npf = fly.hormone_level(tfly::hormone::NEUROPEPTIDE_F);
    let dark = fly.sensory_level(tfly::sense::DARK);
    let terms = vec![
        GateTerm {
            key: "base".to_owned(),
            name: "базовый уровень".to_owned(),
            contribution: 0.15,
            level: 1.0,
        },
        GateTerm {
            key: "dopamine".to_owned(),
            name: "дофамин".to_owned(),
            contribution: 0.55 * dopamine,
            level: dopamine,
        },
        GateTerm {
            key: "octopamine".to_owned(),
            name: "октопамин".to_owned(),
            contribution: 0.45 * octopamine,
            level: octopamine,
        },
        GateTerm {
            key: "neuropeptide_f".to_owned(),
            name: "нейропептид F".to_owned(),
            contribution: 0.25 * npf,
            level: npf,
        },
        GateTerm {
            key: "serotonin".to_owned(),
            name: "серотонин".to_owned(),
            contribution: -0.35 * serotonin,
            level: serotonin,
        },
        GateTerm {
            key: "dark".to_owned(),
            name: "темнота".to_owned(),
            contribution: 0.20 * dark,
            level: dark,
        },
    ];
    // The core clamps the gate to 0..1, and the raw terms go past both ends
    // whenever the modulators pile up or the serotonin drowns them. Without
    // showing that, the list reads as an explanation while adding up to
    // something other than the number it claims to explain, so the clamp is
    // stated as the term it is.
    let raw: f32 = terms.iter().map(|term| term.contribution).sum();
    let gate = fly.learning_gate();
    let mut terms = terms;
    if (raw - gate).abs() > 1e-4 {
        terms.push(GateTerm {
            key: "clamp".to_owned(),
            name: if raw > gate {
                "срез сверху"
            } else {
                "срез снизу"
            }
            .to_owned(),
            contribution: gate - raw,
            level: 0.0,
        });
    }
    terms
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
    // ---- the course ----
    //
    // One maze per act, each rebuilt when its round ends. They are built eagerly
    // for all five acts so switching acts does not stall, and each keeps its own
    // seed so a troupe spread over the circus is not all solving the same puzzle.
    courses: Vec<Course>,
    /// The collision circles of each course, in arena coordinates. Derived from
    /// the course once, when it is built, rather than every tick.
    course_solids: Vec<Vec<[f32; 3]>>,
    /// The round seed the per-character solid lists were last built for, and the
    /// act they were built for. While these still match, nobody has moved scenery
    /// and the lists are rebuilt not at all.
    solids_built_for: u32,
    solids_built_act: usize,
    /// Rounds finished per character.
    scores: Vec<u32>,
    /// Which round the troupe is on.
    round: u32,
    /// Seconds left in this round.
    round_time: f32,
    /// Seconds of finish left to show before the maze is rebuilt, or zero.
    feed_pause: f32,
    /// The seed the current round was built from, so it can be repeated.
    round_seed: u32,
    /// One trained brain per fly, indexed like `agents`.
    brains: Vec<Brain>,
    /// Which act each fly is currently in.
    acts: Vec<usize>,
    /// The act the runtime is broadcasting to.
    current_act: usize,
    /// Epsilon for exploration while training.
    epsilon: f32,
    /// Learning rate as a multiple of the core's default, applied to every fly.
    /// The default is honest and slow: a lesson takes about a thousand trials,
    /// which is a real property of the three-factor rule at a realistic
    /// modulator, and worth being able to watch at both ends of that.
    learn_rate: f32,
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
    /// Smoothed success rate over recent trials. This is how often she picks
    /// right, not how much she has learnt; the association weight is that.
    hit_rate: f32,
    trials: u32,
    correct: u32,
    /// Experience points, awarded for correct trials.
    xp: u32,
    level: u32,
    /// The last lesson the fly attempted.
    last_lesson: &'static str,
    last_outcome: String,
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
    /// Smoothed change in the association weight per trial, used to project how
    /// many trials are left. Measured rather than derived from the core's
    /// constants, because the constants are not the whole story: the eligibility
    /// trace and the modulator both move the real rate.
    weight_rate: f32,
    /// The weight the previous trial finished on, so the per-trial change can be
    /// measured without keeping the whole history.
    weight_last: f32,
}

impl Brain {
    fn new(seed: u32, id: u32) -> Self {
        let mut fly = tfly::Fly::new();
        fly.seed(seed);
        Self {
            fly,
            hit_rate: 0.0,
            trials: 0,
            correct: 0,
            xp: 0,
            level: 1,
            last_lesson: "",
            last_outcome: "ожидание".to_owned(),
            thought: "муха ещё ничего не пробовала".to_owned(),
            history: std::collections::VecDeque::with_capacity(HISTORY_LEN),
            mastered: [false; 5],
            id,
            last_level_log: f32::NEG_INFINITY,
            weight_rate: 0.0,
            weight_last: 0.0,
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
                text: format!("уровень {}", new_level),
            });
        }
        self.level = new_level;
        // Smoothed, so a single bad trial does not erase progress. This is the
        // hit rate and nothing else: it says how often she picks right, not how
        // much she has learnt. The panel shows it next to the association
        // weight, and the two disagree for most of a lesson, because a policy
        // reading a flat matrix keeps guessing correctly by luck. Calling this
        // "mastery" made that look like a contradiction instead of the honest
        // fact that succeeding and learning are different things.
        self.hit_rate = self.hit_rate * 0.9 + f32::from(hit) * 0.1;
        self.last_lesson = act.id;
        self.last_outcome = match result.outcome {
            tfly::TrialOutcome::Correct => "верно",
            tfly::TrialOutcome::Partial => "почти",
            tfly::TrialOutcome::Wrong => "ошибка",
        }
        .to_owned();

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
                    text: format!("забыла урок «{}» — вес упал ниже половины", act.lesson),
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

        // Measure how fast the weight is actually moving, so the panel can say
        // how many trials are left instead of leaving the reader to guess.
        // Smoothed, because one trial's change depends on whether the guess was
        // right and on where the modulator happened to be.
        let moved = weight - self.weight_last;
        self.weight_last = weight;
        self.weight_rate = self.weight_rate * 0.98 + moved * 0.02;

        self.thought = self.compose_thought(act);
        events
    }

    /// Trials still needed to reach a weight of 0.9, projected from the rate the
    /// weight has actually been moving at.
    ///
    /// This is an estimate, and the panel says so. It is measured rather than
    /// derived from the core's constants because the constants are not the whole
    /// story: the eligibility trace and the modulator both move the real rate,
    /// and both change from trial to trial. `None` means the weight is not
    /// moving at all, which is the case worth explaining rather than quoting a
    /// number of trials for.
    fn trials_to_learned(&self, act: &Act) -> Option<u32> {
        let weight = self.mastery_for(act);
        if weight >= 0.9 {
            return Some(0);
        }
        if self.weight_rate <= 1e-6 {
            return None;
        }
        let left = 0.9 - weight;
        Some((left / self.weight_rate).ceil().max(0.0) as u32)
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
            return "сплю, консолидирую ассоциации".to_owned();
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
            courses: Vec::new(),
            course_solids: Vec::new(),
            solids_built_for: u32::MAX,
            solids_built_act: usize::MAX,
            scores: vec![0; count],
            round: 0,
            round_time: 0.0,
            feed_pause: 0.0,
            round_seed: 0,
            brains: (0..count)
                .map(|index| Brain::new(0x5EED + index as u32, index as u32 + 1))
                .collect(),
            acts: vec![0; count],
            current_act: 0,
            epsilon: 0.25,
            learn_rate: 1.0,
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
        // The courses are built here rather than lazily on first use, so the very
        // first frame already has a maze under everyone's feet. `new` cannot call
        // `build_courses`, because that needs `&mut self` on a value that is
        // still being built, so the two steps are written out.
        .with_courses(0x0C0FFEE)
    }

    /// Build the opening courses and put everyone at an entrance.
    fn with_courses(mut self, seed: u32) -> Self {
        self.round = 1;
        self.round_time = ROUND_LENGTH;
        self.build_courses(seed);
        self
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
        // Her scenery just changed underneath her, so the cached lists of every
        // character on the old act are stale. Saying so is one store; the lists
        // themselves are rebuilt on the next tick.
        self.solids_built_for = u32::MAX;
        let brain = &mut self.brains[agent_index];
        brain.fly.seed(0x5EED + agent_index as u32);
        brain.fly.clear_hormones();
        present_act(&mut brain.fly, act);
    }

    /// How high the surface is under a character, and what she is standing
    /// inside of.
    ///
    /// A stage is a raised disc, so a character standing on one is standing on
    /// top of it. Before this she was placed at a flat zero, which put her
    /// ankle-deep inside every stage she walked onto.
    ///
    /// The height is eased rather than snapped, because a stage edge is a step
    /// and a step is taken over a few frames. Snapping it teleports her.
    fn resolve_ground(&mut self, dt: f32) {
        for i in 0..self.agents.len() {
            let act = &ACTS[self.acts[i].min(ACTS.len() - 1)];
            let dx = self.agents[i].position[0] - act.origin[0];
            let dy = self.agents[i].position[1] - act.origin[1];
            let on_stage = (dx * dx + dy * dy).sqrt() < act.stage_radius;
            let target = if on_stage { act.stage_height } else { 0.0 };
            // Stepping up onto a stage is quicker than stepping down off one,
            // because a person pushes up and then settles.
            let tau = if target > self.agents[i].ground {
                0.06
            } else {
                0.16
            };
            let k = 1.0 - (-dt / tau).exp();
            self.agents[i].ground += (target - self.agents[i].ground) * k;
        }
    }

    /// Push characters out of the solids standing on their stage.
    ///
    /// The stage scenery is built from the same numbers this reads, so a wall
    /// and its collider cannot end up in different places. Solids off the stage
    /// are ignored: a character on the tent floor is not inside a stage post
    /// that happens to be nearby.
    /// Tell each body what is solid around it.
    ///
    /// The scenery and the colliders come from the same numbers, so a wall and
    /// its collider cannot end up in different places. The resolution itself
    /// lives in the body, where the movement happens: a pass running after the
    /// movement has already happened cannot help, because the steering
    /// recomputes the velocity every tick and discards what an outside pass
    /// wrote to it.
    /// Put the right solids under everybody's feet.
    ///
    /// Only when something has actually changed. The course is rebuilt once a
    /// round and nobody moves scenery otherwise, so doing this every tick meant
    /// allocating a fresh list of a few hundred circles per character per frame
    /// and throwing it away sixty times a second: the single largest source of
    /// garbage in the runtime, for a result identical to the last one.
    fn refresh_solids(&mut self) {
        if self.solids_built_for == self.round_seed && self.solids_built_act == self.current_act {
            for (i, act_index) in self.acts.iter().enumerate() {
                let act = &ACTS[(*act_index).min(ACTS.len() - 1)];
                self.agents[i].stage = [act.origin[0], act.origin[1], act.stage_radius];
            }
            return;
        }
        for i in 0..self.agents.len() {
            let act_index = self.acts[i].min(ACTS.len() - 1);
            let act = &ACTS[act_index];
            self.agents[i].stage = [act.origin[0], act.origin[1], act.stage_radius];
            let world = |c: &Collider| [act.origin[0] + c.x, act.origin[1] + c.z, c.radius];
            let list = &mut self.agents[i].solids;
            list.clear();
            if self.course_solids.get(act_index).is_some() {
                // The course replaces the act's own scenery. The labyrinth's four
                // ring walls *were* the maze, and left in place under a real maze
                // they cut across it: two walls a body apart cannot be resolved by
                // pushing, and she ends up standing inside one of them. The corner
                // posts every stage shares stay, because they are outside the maze
                // and are the edge of the stage.
                list.extend(CORNER_POSTS.iter().map(world));
            } else {
                list.extend(act.colliders.iter().filter(|c| c.radius > 0.0).map(world));
            }
            if let Some(course) = self.course_solids.get(act_index) {
                list.extend_from_slice(course);
            }
            list.shrink_to_fit();
        }
        self.solids_built_for = self.round_seed;
        self.solids_built_act = self.current_act;
    }

    fn feed_ground_speed(&mut self, before: &[[f32; 2]]) {
        for i in 0..self.agents.len() {
            if let Some(prev) = before.get(i) {
                let dx = self.agents[i].position[0] - prev[0];
                let dy = self.agents[i].position[1] - prev[1];
                let covered = (dx * dx + dy * dy).sqrt();
                // Speed is the last tick's distance over the tick, not a running
                // average: an average would lag a turn and report motion after
                // she has stopped.
                self.brains[i].fly.ground_speed(covered / FIXED_DT);
            }
        }
    }

    /// Keep characters out of each other's personal space.
    ///
    /// Two characters standing inside one another reads as a bug, and it also
    /// makes the encounter geometry meaningless, because there is no "near" to
    /// speak of. The push is soft and scales with how far inside the space
    /// they are, so they ease apart rather than shooting apart. A frightened
    /// character wants more room than a comfortable one, which is why the
    /// radius is read from the brain rather than fixed.
    fn resolve_personal_space(&mut self, dt: f32) {
        for i in 0..self.agents.len() {
            for j in (i + 1)..self.agents.len() {
                let dx = self.agents[j].position[0] - self.agents[i].position[0];
                let dy = self.agents[j].position[1] - self.agents[i].position[1];
                let distance = (dx * dx + dy * dy).sqrt();
                let wanted = self.space_wanted(i) + self.space_wanted(j);
                if (distance >= wanted) || (distance < 1e-4) {
                    continue;
                }
                // Push each one out along the line between them, and split the
                // correction so neither is shoved.
                let push = (wanted - distance) * dt * 2.5;
                let ux = dx / distance;
                let uy = dy / distance;
                self.agents[i].velocity[0] -= ux * push;
                self.agents[i].velocity[1] -= uy * push;
                self.agents[j].velocity[0] += ux * push;
                self.agents[j].velocity[1] += uy * push;
            }
        }
    }

    /// How much room a character wants around her, in world units.
    fn space_wanted(&self, index: usize) -> f32 {
        let brain = &self.brains[index];
        let shy = brain.fly.emotion_level(tfly::emotion::SHYNESS);
        let fear = brain
            .fly
            .emotion_level(tfly::emotion::FEAR)
            .max(brain.fly.fear_level());
        let bond = brain.fly.bond();
        // Shyness and fear open the circle; a bond closes it, because
        // somebody you know is somebody you will happily stand close to.
        PERSONAL_SPACE * (1.0 + 0.45 * shy + 0.6 * fear - 0.3 * bond)
    }

    /// Let each character look at whoever it last met.
    ///
    /// The core already turns its gaze toward a mate, but only as a bias with
    /// no idea where the mate actually is. Pointing the world coupling at the
    /// partner's real position is what turns that bias into a look at someone
    /// rather than a glance off to one side.
    fn aim_gaze_at_partners(&mut self) {
        for index in 0..self.agents.len() {
            let Some(partner_id) = self.partners.get(index).copied().flatten() else {
                continue;
            };
            let Some(other) = self.agents.iter().position(|agent| agent.id == partner_id) else {
                continue;
            };
            if other == index {
                continue;
            }
            let (ax, ay) = (
                self.agents[index].position[0],
                self.agents[index].position[1],
            );
            let (bx, by) = (
                self.agents[other].position[0],
                self.agents[other].position[1],
            );
            let dx = bx - ax;
            let dy = by - ay;
            let distance = (dx * dx + dy * dy).sqrt();
            // Only worth looking at from close enough to see a face.
            if !(0.05..4.0).contains(&distance) {
                continue;
            }
            // The coupling is a normalised attractor, so clamp rather than
            // divide: a partner at arm's length and one across the stage both
            // mean "over there".
            let nx = (dx / distance).clamp(-1.0, 1.0) * (distance.min(2.0) / 2.0);
            let ny = (dy / distance).clamp(-1.0, 1.0) * (distance.min(2.0) / 2.0);
            self.brains[index].fly.look_at(nx, ny);
            // Looking at someone is looking away from your own goal, so the
            // act pull has to give way or she will walk through them.
            self.brains[index].fly.look_strength(0.6);
            self.brains[other].fly.look_at(-nx, -ny);
            self.brains[other].fly.look_strength(0.6);
        }
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

        // Walk for a fixed window and see what happens. The body is advanced so
        // the distance is real rather than zero, and then put back where it was.
        //
        // Putting it back is the whole point. A gait trial is a measurement, not
        // a journey: it asks how far this gait carries her over a fixed window and
        // nothing else. Leaving her where the trial ended moved her half a unit in
        // a straight line twice a second, straight past every turn she had decided
        // on, which on a course threw her into the wrong corridor several times a
        // round. Where she actually is belongs to the course, not to the trainer.
        brain.fly.act(tfly::action::FORWARD, 1.0);
        brain.fly.steps(40, FIXED_DT);
        self.agents[index].advance(40, FIXED_DT);
        let energy_used = (energy_before - brain.fly.energy()).max(0.0);
        let fell = brain.fly.balance() < 0.3;

        let end = self.agents[index].position;
        let walked = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
        self.agents[index].position = start;
        self.agents[index].velocity = [0.0, 0.0, 0.0];
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

    /// Mean mastery across every fly, from the weight each has on the act it is
    /// in. Averaging the hit rate instead would average a different quantity,
    /// and would report a troupe that guesses well as a troupe that has learnt
    /// something.
    fn average_mastery(&self) -> f32 {
        if self.brains.is_empty() {
            return 0.0;
        }
        let total: f32 = self
            .brains
            .iter()
            .enumerate()
            .map(|(index, brain)| {
                let act = &ACTS[self.acts[index].min(ACTS.len() - 1)];
                brain.mastery_for(act)
            })
            .sum();
        total / self.brains.len() as f32
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
                eye_open: 1.0,
                eye_adapt: 1.0,
                wake_timer: 99.0,
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
            eye_open: brain.fly.eye_open(),
            eye_adapt: brain.fly.eye_adapt(),
            wake_timer: brain.fly.wake_timer(),
        }
    }

    /// Posture, straight from the brain.
    fn posture_for(&self, id: u32) -> PostureSnapshot {
        let Some(brain) = self.brain_of(id) else {
            return PostureSnapshot {
                spine: 0.0,
                shoulder: 0.0,
                lean: 0.0,
            };
        };
        PostureSnapshot {
            spine: brain.fly.spine(),
            shoulder: brain.fly.shoulder(),
            lean: brain.fly.lean(),
        }
    }

    /// Every joint, straight from the brain.
    fn limbs_for(&self, id: u32) -> LimbSnapshot {
        let Some(brain) = self.brain_of(id) else {
            let rest = JointsSnapshot {
                root: 0.0,
                middle: 0.0,
                end: 0.0,
                spread: 0.0,
            };
            return LimbSnapshot {
                leg_l: rest,
                leg_r: rest,
                arm_l: rest,
                arm_r: rest,
                foot_drop: -(RIG_LEG.thigh + RIG_LEG.shin + RIG_LEG.sole_c + RIG_LEG.sole_a),
            };
        };
        let pose = brain.fly.pose();
        let joints = |j: tfly::Joints| JointsSnapshot {
            root: j.root,
            middle: j.middle,
            end: j.end,
            spread: j.spread,
        };
        LimbSnapshot {
            leg_l: joints(pose.legs[0]),
            leg_r: joints(pose.legs[1]),
            arm_l: joints(pose.arms[0]),
            arm_r: joints(pose.arms[1]),
            foot_drop: brain.fly.foot_drop(&RIG_LEG),
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

    /// Every emotion for a fly, strongest first, with a display name and a
    /// stable key for each.
    fn affect_for(&self, id: u32) -> Vec<EmotionReading> {
        let Some(brain) = self.brain_of(id) else {
            return Vec::new();
        };
        let mut pairs: Vec<EmotionReading> = brain
            .fly
            .affect()
            .into_iter()
            .map(|(emotion_id, _, value)| EmotionReading {
                name: EMOTION_NAMES
                    .get(emotion_id as usize)
                    .copied()
                    .unwrap_or("?")
                    .to_owned(),
                key: EMOTION_KEYS
                    .get(emotion_id as usize)
                    .copied()
                    .unwrap_or("unknown"),
                value,
            })
            .collect();
        pairs.sort_by(|a, b| {
            b.value
                .partial_cmp(&a.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
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
                    stage_radius: act.stage_radius,
                    stage_height: act.stage_height,
                    colliders: act.colliders,
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
            learning: format!("{} → {}", cue_name(act.cue), action_name(act.solution)),
            mood: EMOTION_NAMES
                .get(brain.fly.dominant_emotion_id().max(0) as usize)
                .copied()
                .unwrap_or("?")
                .to_owned(),
            drive: tfly::drive::NAMES
                .get(brain.fly.dominant_drive_id().max(0) as usize)
                .copied()
                .unwrap_or("?")
                .to_owned(),
            thought: brain.thought.clone(),
            mastery: brain.mastery_for(act),
            hit_rate: brain.hit_rate,
            trials: brain.trials,
            correct: brain.correct,
            xp: brain.xp,
            level: brain.level,
            weight: brain.fly.weight(act.cue, act.solution),
            gate: brain.fly.learning_gate(),
            gate_terms: gate_terms(&brain.fly),
            trials_left: brain.trials_to_learned(act),
            learn_rate: brain.fly.learn_rate(),
            plasticity: brain.fly.plasticity_level(),
            valence: brain.fly.valence(),
            arousal: brain.fly.arousal(),
            pain: brain.fly.pain_total(),
            energy: brain.fly.energy(),
            stress: brain.fly.stress(),
            memory: brain.fly.memory_count(),
            last_lesson: brain.last_lesson.to_owned(),
            last_outcome: brain.last_outcome.clone(),
            curve: brain.history.iter().cloned().collect(),
        })
    }

    /// Build every act's course, and put everyone on the one for their act.
    ///
    /// The five acts get five different seeds, so a troupe spread across the
    /// circus is not all looking at the same maze. The seed the current round was
    /// built from is kept, because a round has to be repeatable or a bug report
    /// is a description of a maze nobody else will ever see.
    fn build_courses(&mut self, seed: u32) {
        self.courses = (0..ACTS.len())
            .map(|index| {
                let act = &ACTS[index];
                if act.maze_cells == 0 {
                    // Open ground, such as the hill a fly wakes up on. There is
                    // nothing here to be lost in, and that is the point of it.
                    return Course::open(act.stage_radius);
                }
                Course::build(
                    seed.wrapping_add(0x9E37_79B9u32.wrapping_mul(index as u32 + 1)),
                    act.maze_cells,
                    act.stage_radius,
                )
            })
            .collect();
        self.course_solids = self
            .courses
            .iter()
            .enumerate()
            .map(|(index, course)| {
                let [ax, ay] = ACTS[index].origin;
                course
                    .solids()
                    .into_iter()
                    .map(|[x, z, r]| [ax + x, ay + z, r])
                    .collect()
            })
            .collect();
        self.round_seed = seed;
        self.refresh_solids();
        self.send_everyone_to_the_start();
    }

    /// Put every character at the entrance of their own act's course.
    fn send_everyone_to_the_start(&mut self) {
        for index in 0..self.agents.len() {
            let act_index = self.acts[index].min(ACTS.len() - 1);
            let Some(course) = self.courses.get(act_index) else {
                continue;
            };
            // A fan of a few points around the entrance, so they do not start
            // inside one another and the first thing on screen is not a pile.
            let spread = index as f32 * 1.7;
            let [ax, ay] = ACTS[act_index].origin;
            self.agents[index].position[0] = ax + course.start[0] + 0.10 * spread.cos();
            self.agents[index].position[1] = ay + course.start[1] + 0.10 * spread.sin();
            self.agents[index].position[2] = 0.0;
            self.agents[index].velocity = [0.0, 0.0, 0.0];
            self.agents[index].fed = false;
            self.agents[index].ate_feed = usize::MAX;
            self.agents[index].ate_count = 0;
            self.agents[index].trail.clear();
            // The new maze is a new place. What she worked out about the last one
            // is worth exactly nothing here, and keeping it would be the worst
            // possible bug: she would walk confidently to a wall that is not there.
            self.agents[index].search =
                (!course.passages.is_empty()).then(|| Search::new(&course.passages));
            self.agents[index].nav = [ax + course.start[0], ay + course.start[1]];
            self.agents[index].nav_weight = 0.0;
        }
    }

    /// Has a character reached the food on her course?
    ///
    /// Returns true only on the tick it happens, so the caller can pay out once.
    /// The reward goes into the brain, not into a counter, because a character
    /// that found the food should look like it: the pulse is what the face and
    /// the posture are reading.
    fn try_feed(&mut self, index: usize) -> bool {
        let act_index = self.acts[index].min(ACTS.len() - 1);
        let Some(course) = self.courses.get(act_index) else {
            return false;
        };
        if self.agents[index].fed {
            return false;
        }
        let [ax, ay] = ACTS[act_index].origin;
        // The nearest food she has not had yet. Foods are laid out nearest-first,
        // so this is the first one deeper than the deepest she has, and the round
        // is over for her when there is no deeper one left.
        let held = if self.agents[index].ate_feed == usize::MAX {
            0
        } else {
            self.agents[index].ate_feed + 1
        };
        let Some(feed) = course.feeds.get(held) else {
            return false;
        };
        let dx = self.agents[index].position[0] - (ax + feed[0]);
        let dy = self.agents[index].position[1] - (ay + feed[1]);
        if dx.hypot(dy) > FOOD_REACH {
            return false;
        }
        let deeper =
            self.agents[index].ate_feed == usize::MAX || held > self.agents[index].ate_feed;
        if deeper {
            self.agents[index].ate_feed = held;
            self.agents[index].ate_count += 1;
        }
        self.learning_updates = self.learning_updates.saturating_add(1);
        let last = held + 1 >= course.feeds.len();
        if last {
            self.agents[index].fed = true;
        }
        let id = self.agents[index].id;
        let act = ACTS[act_index].name;
        let text = if course.feeds.len() <= 1 {
            format!(
                "дошла до еды на «{act}» за {} с",
                (ROUND_LENGTH - self.round_time).max(0.0).round()
            )
        } else if last {
            format!(
                "съела всю еду на «{act}» ({}) за {} с",
                course.feeds.len(),
                (ROUND_LENGTH - self.round_time).max(0.0).round()
            )
        } else {
            format!(
                "еда {}/{} на «{act}», за {} с",
                held + 1,
                course.feeds.len(),
                (ROUND_LENGTH - self.round_time).max(0.0).round()
            )
        };
        self.log(LogEntry {
            t: 0.0,
            fly: id,
            kind: "fed".to_owned(),
            text,
        });
        true
    }

    /// The courses as the client needs them: one per act.
    ///
    /// Stage-local, like the act's colliders, so the client places each on the
    /// stage it already builds rather than having to know where the stages are.
    fn course_snapshot(&self) -> Vec<CourseSnapshot> {
        (0..ACTS.len()).map(|act| self.one_course(act)).collect()
    }

    fn one_course(&self, act_index: usize) -> CourseSnapshot {
        let round = self.round;
        let seed = self.round_seed;
        let time_left = self.round_time;
        let Some(course) = self.courses.get(act_index) else {
            return CourseSnapshot {
                act: act_index,
                round,
                seed,
                time_left,
                round_length: ROUND_LENGTH,
                food: [0.0, 0.0],
                feeds: Vec::new(),
                cell: 0.0,
                start: [0.0, 0.0],
                path_length: 0.0,
                walls: Vec::new(),
                runners: Vec::new(),
                fed: 0,
                count: 0,
            };
        };
        let [ax, ay] = ACTS[act_index].origin;
        let runners: Vec<RunnerSnapshot> = self
            .agents
            .iter()
            .enumerate()
            .filter(|(index, _)| self.acts[*index].min(ACTS.len() - 1) == act_index)
            .map(|(index, agent)| {
                // Stage-local, because the client draws on the stage.
                let distance = (agent.position[0] - ax - course.goal[0])
                    .hypot(agent.position[1] - ay - course.goal[1]);
                let worst = course
                    .path_length
                    .max((course.goal[0] - course.start[0]).hypot(course.goal[1] - course.start[1]))
                    .max(0.001);
                RunnerSnapshot {
                    id: agent.id,
                    fed: agent.fed,
                    ate_feed: agent.ate_feed,
                    ate_count: agent.ate_count,
                    feeds_total: course.feeds.len() as u32,
                    distance,
                    progress: (1.0 - distance / worst).clamp(-1.0, 1.0),
                    score: self.scores[index],
                    // A character who is out of the round has nothing left to show,
                    // and a trail she no longer needs costs the client a draw every
                    // frame.
                    trail: agent.trail.iter().map(|p| [p[0] - ax, p[1] - ay]).collect(),
                }
            })
            .collect();
        CourseSnapshot {
            act: act_index,
            round,
            seed,
            time_left,
            round_length: ROUND_LENGTH,
            food: course.feeds.first().copied().unwrap_or(course.goal),
            feeds: course.feeds.clone(),
            cell: course.cell,
            start: course.start,
            path_length: course.path_length,
            walls: course
                .walls
                .iter()
                .map(|w| WallSnapshot {
                    x: w.x,
                    z: w.z,
                    half_len: w.half_len,
                    half_thick: w.half_thick,
                    angle: w.angle,
                })
                .collect(),
            fed: runners.iter().filter(|r| r.fed).count() as u32,
            count: runners.len() as u32,
            runners,
        }
    }

    /// Give a character something to walk towards.
    ///
    /// Two pulls, and neither of them is the route. The first is the smell of the
    /// food, which falls off with distance and is therefore weak from across the
    /// stage and strong in the last corridor: enough to aim her across the open
    /// floor, not enough to walk her round a wall. The second is the freshest
    /// breadcrumb any character who has already eaten left behind, which is the
    /// only thing one of them teaches another. Handing over the solved route
    /// instead would make her walk it in a straight line and look like a cursor.
    /// Give a character something to walk towards.
    ///
    /// Three pulls, in order of how far each is trusted.
    ///
    /// The first is what she remembers: the next place worth trying, which is a
    /// place she has not been if there is one, and failing that the way back to
    /// the last place that still had somewhere new to go. This is what makes her
    /// finish. Feeling along the walls on her own does not: she reaches a dead end,
    /// the way back smells exactly like the way on, and she works the same three
    /// metres of passage for as long as the round lasts.
    ///
    /// The second is the smell of the food, which is her initial heading on a
    /// course she has not started yet, and a small lean once she is nearly
    /// somewhere worth standing.
    ///
    /// The third is the freshest breadcrumb any character who has already eaten
    /// left behind, which is the only thing one of them teaches another. Handing
    /// over the solved route instead would make her walk it in a straight line and
    /// look like a cursor.
    fn navigate(&mut self, index: usize) -> [f32; 2] {
        let act_index = self.acts[index].min(ACTS.len() - 1);
        let [ax, ay] = ACTS[act_index].origin;
        let Some(course) = self.courses.get(act_index) else {
            return [ax, ay];
        };
        let me = [
            self.agents[index].position[0],
            self.agents[index].position[1],
        ];
        let local = [me[0] - ax, me[1] - ay];
        // The nearest food, not the far one. The far one is the goal of a maze
        // nobody finishes in a round, and walking towards it is walking away from
        // the only thing that ends one.
        let food = course.feeds.first().copied().unwrap_or(course.goal);
        let to_food = [food[0] - local[0], food[1] - local[1]];
        let food_distance = to_food[0].hypot(to_food[1]).max(1e-4);
        // Falls off with the square of the distance, so the pull is a gradient to
        // climb rather than a direction to obey.
        let smell = 1.0 / (1.0 + food_distance * food_distance * 1.6);
        let smell_dir = [to_food[0] / food_distance, to_food[1] / food_distance];

        // What she remembers, if she can name where she is standing.
        let mut remembered: Option<[f32; 2]> = None;
        if let Some(food_cell) = course.cell_at(food) {
            let here = course.cell_at(local);
            let agent = &mut self.agents[index];
            if agent.search.is_none() && !course.passages.is_empty() {
                agent.search = Some(Search::new(&course.passages));
            }
            if let (Some(search), Some(here)) = (agent.search.as_mut(), here) {
                search.arrive(here);
                // A promise is kept when her feet are in the middle of the place
                // she promised, not when the doorway renames her.
                if let Some(going) = search.committing
                    && let Some(centre) = course.centres.get(going)
                {
                    let d = (centre[0] - local[0]).hypot(centre[1] - local[1]);
                    if d < course.cell * 0.3 {
                        search.committing = None;
                    }
                }
                if let Some(next) = search.next(food_cell) {
                    remembered = course.centres.get(next).copied();
                }
            }
        }
        // A remembered place is one cell away at most, so the smell only decides
        // which side of the cell to aim for. A place already proved useless must
        // not drag her back in on the strength of a whiff of dinner.
        if let Some(target) = remembered {
            self.agents[index].nav_is_place = true;
            let d = (target[0] - local[0]).hypot(target[1] - local[1]).max(1e-4);
            let lean = (1.0 - (d / course.cell).min(1.0)) * (smell * 3.0).min(0.4);
            let ux = (target[0] - local[0]) / d * (1.0 - lean) + smell_dir[0] * lean;
            let uy = (target[1] - local[1]) / d * (1.0 - lean) + smell_dir[1] * lean;
            let len = ux.hypot(uy).max(1e-4);
            return [ax + local[0] + ux / len, ay + local[1] + uy / len];
        }

        // The freshest crumb from anyone who has eaten, on this act.
        let mut lure = [0.0f32; 2];
        let mut lure_strength = 0.0f32;
        for (other, agent) in self.agents.iter().enumerate() {
            if other == index || !agent.fed {
                continue;
            }
            if self.acts[other].min(ACTS.len() - 1) != act_index {
                continue;
            }
            let Some(point) = agent.trail.back() else {
                continue;
            };
            let dx = point[0] - me[0];
            let dy = point[1] - me[1];
            let near = (dx * dx + dy * dy).sqrt().max(1e-4);
            // A crumb only counts if it is nearer than the food is, so a trail
            // does not drag her back the way she came.
            let strength = smell * (1.0 - (near / food_distance).min(1.0)) * 0.8;
            if strength > lure_strength {
                lure_strength = strength;
                lure = [point[0] + dx / near * 0.25, point[1] + dy / near * 0.25];
            }
        }
        self.agents[index].nav_is_place = false;
        self.agents[index].nav_walls = !course.open && !course.walls.is_empty();
        if lure_strength <= 0.0 {
            return [ax + local[0] + smell_dir[0], ay + local[1] + smell_dir[1]];
        }
        let ld = (lure[0] - me[0]).hypot(lure[1] - me[1]).max(1e-4);
        let w = lure_strength.min(0.85);
        let ux = smell_dir[0] * (1.0 - w) + (lure[0] - me[0]) / ld * w;
        let uy = smell_dir[1] * (1.0 - w) + (lure[1] - me[1]) / ld * w;
        let len = ux.hypot(uy).max(1e-4);
        [ax + local[0] + ux / len, ay + local[1] + uy / len]
    }

    pub fn step(&mut self, dt: f32) {
        if !self.running {
            return;
        }
        self.time += dt * self.speed;
        self.tick += 1;
        // Where everyone stood before this tick, so the ground speed can be
        // measured from the displacement rather than believed from the steering.
        let before: Vec<[f32; 2]> = self
            .agents
            .iter()
            .map(|a| [a.position[0], a.position[1]])
            .collect();
        for index in 0..self.agents.len() {
            let act = &ACTS[self.acts[index].min(ACTS.len() - 1)];
            let [ax, ay] = act.origin;
            self.agents[index].step(self.time, dt, self.speed, self.decay);
            let act = &ACTS[self.acts[index].min(ACTS.len() - 1)];
            self.agents[index].orbit_room = orbit_room(act);
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
            // She has a course to run, so she is pointed at it and stops merely
            // circling her stage. A character who has eaten is out of the round
            // and goes back to milling, which reads as finished rather than as
            // somebody who forgot where she was going.
            let act_index = self.acts[index].min(ACTS.len() - 1);
            if self.courses.get(act_index).is_some() && !self.agents[index].fed {
                let nav = self.navigate(index);
                self.agents[index].nav = nav;
                self.agents[index].nav_weight = 1.0;
            } else {
                self.agents[index].nav_weight = 0.0;
                self.agents[index].nav_is_place = false;
            }
            self.agents[index].steer_towards([ax, ay], 1.2, dt * self.speed);
            self.total_reactions += 1;
            if self.agents[index].hormone_level > 0.25 {
                self.total_hormones += 1;
            }
            if self.agents[index].puff_just_started {
                self.total_puffs = self.total_puffs.saturating_add(1);
            }
            if self.try_feed(index) {
                // Reaching the food is the one thing in the runtime worth a
                // reward, so it goes into the brain rather than into a counter the
                // client keeps: she gets a pulse, and the character that finds it
                // first is the one that looks rewarded.
                self.brains[index].fly.reward(0.9);
                self.agents[index].reward = 1.0;
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
        // Then look at whoever was just met, and keep out of each other's way.
        // Both run every frame rather than on the encounter cadence, because
        // two characters drifting together need easing apart the whole time
        // they are close, not once every encounter tick.
        self.aim_gaze_at_partners();
        self.resolve_personal_space(dt);
        // And then the walls again, because easing two characters apart is a
        // displacement of its own and it can push one of them into a wall. The
        // body resolved its own movement before the others moved it, so without
        // this a shove is the one thing that puts somebody inside the scenery.
        for agent in &mut self.agents {
            if agent.slide_out_of_solids() > JAMMED {
                agent.wall_timer = WALL_COMMITMENT;
            }
        }
        // Scenery last: a character has already been pushed by the others, and
        // the wall is the harder boundary of the two.
        self.refresh_solids();
        self.resolve_ground(dt);
        // Measured after everything has moved her, so it reports where she
        // actually ended up rather than where she was aiming.
        self.feed_ground_speed(&before);

        // The round ends when everyone has eaten or when the clock runs out,
        // whichever comes first, and a new maze is built under their feet. That
        // is the point of rebuilding: a route worked out in the last round is
        // worth nothing in this one, so the round cannot be won twice.
        self.round_time -= dt * self.speed;
        let all_fed = !self.agents.is_empty() && self.agents.iter().all(|a| a.fed);
        if all_fed {
            // Not this tick. The last character to arrive ends the round the
            // instant she touches the food, the maze is rebuilt under the troupe,
            // and everybody is stood back at the entrance before a frame has been
            // drawn. Nobody sees the thing they just won, and the panel has no
            // chance to say who won it. So the round is left standing for a few
            // seconds with the food gone and the score showing, which is what a
            // finish looks like.
            if self.feed_pause <= 0.0 {
                self.feed_pause = FEED_PAUSE;
            }
            self.feed_pause -= dt * self.speed;
        }
        if (all_fed && self.feed_pause <= 0.0) || self.round_time <= 0.0 {
            self.feed_pause = 0.0;
            self.end_round(all_fed);
        }
    }

    /// Close the round, score it, and build the next one.
    fn end_round(&mut self, everyone_ate: bool) {
        for index in 0..self.agents.len() {
            if self.agents[index].fed {
                self.scores[index] = self.scores[index].saturating_add(1);
            }
        }
        let round = self.round;
        let eaten = self.agents.iter().filter(|a| a.fed).count();
        let total = self.agents.len();
        let why = if everyone_ate {
            "все дошли"
        } else if eaten == 0 {
            "никто не дошёл"
        } else {
            "время вышло"
        };
        self.log(LogEntry {
            t: 0.0,
            fly: 0,
            kind: "round".to_owned(),
            text: format!("раунд {round}: {why} ({eaten} из {total})"),
        });
        self.round = round.saturating_add(1).max(1);
        self.round_time = ROUND_LENGTH;
        // A new seed every round, so the course is a new problem each time. The
        // seed advances from the last one rather than being drawn fresh, so a
        // session is reproducible from its first round.
        let next = self
            .round_seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.build_courses(next);
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
                    ground: agent.ground,
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
                    posture: self.posture_for(agent.id),
                    limbs: self.limbs_for(agent.id),
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
            courses: self.course_snapshot(),
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
            // Build a new course now, without waiting for the round to end. The
            // seed is optional, so a round can be asked for by number and
            // repeated exactly.
            "course" => {
                let seed = command
                    .value
                    .map(|v| v.max(0.0) as u32)
                    .unwrap_or_else(|| self.round_seed.wrapping_mul(2_654_435_761).wrapping_add(1));
                self.round = self.round.saturating_add(1).max(1);
                self.round_time = ROUND_LENGTH;
                self.build_courses(seed);
                self.log(LogEntry {
                    t: 0.0,
                    fly: 0,
                    kind: "round".to_owned(),
                    text: format!("новая полоса, seed {seed}"),
                });
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
                        text: format!("«{}»: уровень {}", act.name, self.brains[index].level),
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
            // How fast a lesson takes. A learning rate, not a fourth factor: the
            // modulator still gates every update, so a shut gate still teaches
            // nothing however fast the rate is set.
            "learnrate" => {
                let rate = command.value.unwrap_or(self.learn_rate).clamp(0.1, 20.0);
                self.learn_rate = rate;
                for brain in &mut self.brains {
                    brain.fly.set_learn_rate(rate);
                }
                self.log(LogEntry {
                    t: 0.0,
                    fly: 0,
                    kind: "learnrate".to_owned(),
                    text: format!("скорость обучения x{rate:.1}"),
                });
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
                    brain.hit_rate = 0.0;
                    brain.xp = 0;
                    brain.level = 1;
                    brain.trials = 0;
                    brain.correct = 0;
                    brain.history.clear();
                    brain.mastered = [false; 5];
                    // The measured rate goes too, or the panel keeps quoting a
                    // projection from the learning that was just wiped.
                    brain.weight_rate = 0.0;
                    brain.weight_last = 0.0;
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
                            "походка «{}» освоена на {:.0}%",
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
            // Put a character to sleep, or wake her.
            //
            // This exists because the waking sequence is the most interesting
            // thing the face does and there was no other way to see it: she
            // only ever fell asleep on her own once her energy ran out, which
            // took minutes of simulated time.
            "sleep" => {
                let id = command.id.context("sleep requires id")?;
                if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                    if command.value.map(|v| v > 0.5).unwrap_or(true) {
                        self.brains[index].fly.sleep();
                    } else {
                        self.brains[index].fly.wake();
                    }
                    self.log(LogEntry {
                        t: 0.0,
                        fly: id,
                        kind: "sleep".to_owned(),
                        text: if self.brains[index].fly.is_asleep() {
                            "уснула".to_owned()
                        } else {
                            "проснулась".to_owned()
                        },
                    });
                }
            }
            // Startle a character, which is the fastest way to watch the eyes
            // fly open and the waking sequence play out.
            "startle" => {
                let id = command.id.context("startle requires id")?;
                let amount = command.value.unwrap_or(0.95).clamp(0.0, 1.0);
                if let Some(index) = self.agents.iter().position(|agent| agent.id == id) {
                    self.brains[index].fly.surprise(amount);
                    self.log(LogEntry {
                        t: 0.0,
                        fly: id,
                        kind: "startle".to_owned(),
                        text: "испуг".to_owned(),
                    });
                }
            }
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
                    let mut brain = Brain::new(0x5EED + id, id);
                    // A new character joins at whatever pace the troupe is on.
                    brain.fly.set_learn_rate(self.learn_rate);
                    self.brains.push(brain);
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
    fn emotion_keys_are_the_core_keys_in_the_core_order() {
        // The editor keeps its own copy of the keys so the client can bind to
        // a feeling by a stable name. A copy that is shifted by one still has
        // the right length and the right words in it, so nothing notices until
        // "радость" is filed under "sadness". The C core is the authority: ask
        // it, rather than compare the two copies against each other.
        for (index, key) in EMOTION_KEYS.iter().enumerate() {
            assert_eq!(
                tfly::Fly::emotion_key(index as i32),
                *key,
                "emotion {index} is filed under the wrong key"
            );
        }
    }

    #[test]
    fn emotion_names_are_distinct_and_all_present() {
        // A table of labels is read by people, so two emotions sharing a label
        // is a visible defect, and a placeholder would mean the list is short.
        for (index, name) in EMOTION_NAMES.iter().enumerate() {
            assert!(name.chars().count() > 1, "emotion {index} has no label");
        }
        let mut sorted = EMOTION_NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            EMOTION_NAMES.len(),
            "two emotions share a label"
        );
    }

    #[test]
    fn act_labels_are_distinct() {
        // The same class of defect, on the table a person reads first: five
        // acts whose subtitles all said the same thing would look like a
        // rendering fault and be one.
        let mut names: Vec<&str> = ACTS.iter().map(|act| act.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ACTS.len(), "two acts share a name");

        let mut lessons: Vec<&str> = ACTS.iter().map(|act| act.lesson).collect();
        lessons.sort_unstable();
        lessons.dedup();
        assert_eq!(lessons.len(), ACTS.len(), "two acts share a lesson");
    }

    #[test]
    fn training_and_gait_report_what_they_actually_did() {
        // These two lines are the running commentary on learning, so a message
        // that does not mention the act or the gait is worse than no message.
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"train","value":200}"#)
            .expect("train");
        runtime
            .apply_command(br#"{"action":"walk","value":200}"#)
            .expect("walk");
        let texts: Vec<&str> = runtime
            .log
            .iter()
            .map(|entry| entry.text.as_str())
            .collect();
        for expected in ["уровень", "походка"] {
            assert!(
                texts.iter().any(|text| text.contains(expected)),
                "no log line mentions {expected}: {texts:?}"
            );
        }
        // The gait line must name a real preset, not the array itself.
        let gait_line = texts
            .iter()
            .find(|text| text.contains("походка"))
            .copied()
            .expect("a gait line");
        assert!(
            GAIT_NAMES.iter().any(|preset| gait_line.contains(preset)),
            "the gait line names no preset: {gait_line}"
        );
    }

    #[test]
    fn a_name_from_one_table_never_appears_in_another() {
        // The core keeps a Russian label per gait, per gesture and per
        // encounter. A word that drifted from one of those tables into another
        // is invisible in review and obvious on screen: a character standing
        // still reported the gesture "петляющий", which is a gait, and the
        // seventh encounter reported itself as one too. The three sets of
        // labels have to stay apart.
        let table = |count: i32, name: fn(i32) -> &'static str| -> Vec<String> {
            (0..count).map(|i| name(i).to_owned()).collect()
        };
        let gaits = table(tfly::gait::COUNT, tfly::Fly::gait_name);
        let gestures = table(tfly::gesture::COUNT, tfly::Fly::gesture_name);
        let encounters = table(tfly::encounter::COUNT, tfly::Fly::encounter_name);

        for (mine, mine_name, other, other_name) in [
            (&gestures, "gesture", &gaits, "gait"),
            (&gaits, "gait", &gestures, "gesture"),
            (&encounters, "encounter", &gaits, "gait"),
            (&encounters, "encounter", &gestures, "gesture"),
        ] {
            for label in mine {
                assert!(
                    !other.contains(label),
                    "the {mine_name} label {label:?} is also a {other_name} label"
                );
            }
        }

        // And the core's own view of the gaits has to agree with the editor's
        // copy, which is the list the rig binds to.
        for (index, name) in GAIT_NAMES.iter().enumerate() {
            assert_eq!(
                tfly::Fly::gait_name(index as i32),
                *name,
                "gait {index} is named {name} here and differently in the core"
            );
        }
    }

    /// A lesson for the tests below: bright light, answered by dancing.
    fn test_lesson() -> tfly::Lesson {
        tfly::Lesson {
            id: "test",
            name: "test",
            objective: "test",
            cue: tfly::cue::LIGHT,
            solution: tfly::action::DANCE,
            distractor: tfly::action::REST,
        }
    }

    #[test]
    fn gate_terms_sum_to_the_gate() {
        // The panel shows the modulator as a list of terms so a low gate can be
        // read rather than guessed at. That list is a second copy of the formula
        // in `TLearningGate`, and a second copy is exactly the kind of thing that
        // drifts: someone reweights a hormone in the core, the panel keeps the
        // old numbers, and the breakdown stops explaining the total it claims to
        // explain. This asks the core for the gate and adds up the terms, so a
        // change on either side fails here.
        let mut runtime = EditorRuntime::new(2);
        // Both ends of the clamp have to be covered, because the raw terms
        // leave 0..1 at both ends and a breakdown that ignores that stops
        // explaining the gate precisely when the modulator is extreme. The
        // editor's own chemistry does not reach either end inside a short
        // run, so the fly's modulators are driven directly through the core:
        // the same thing the editor does, and it keeps the test about the
        // formula rather than about pacing.
        let mut clamped_high = false;
        let mut clamped_low = false;
        for step in 0..400 {
            // Saturate the modulators on one side, then the other, so the gate
            // is driven past both bounds and back. Reaching the top needs all
            // four, because the positive terms sum to 1.40 and serotonin takes
            // 0.35 of that back; reaching the bottom needs serotonin alone,
            // which is the only term that can push the raw sum below the base.
            let high = step % 200 < 100;
            for brain in &mut runtime.brains {
                for (hormone, alone) in [
                    (tfly::hormone::SEROTONIN, true),
                    (tfly::hormone::DOPAMINE, false),
                    (tfly::hormone::OCTOPAMINE, false),
                    (tfly::hormone::NEUROPEPTIDE_F, false),
                ] {
                    // Serotonin is held high on its own in the low phase, because
                    // it is the only term that can push the raw sum below the
                    // base, so the bottom of the range is reachable at all.
                    let value = if alone { 1.0 } else { f32::from(high) };
                    brain.fly.set_hormone(hormone, value);
                }
            }
            runtime.step(FIXED_DT);
            let Some(brain) = runtime.brain_snapshot() else {
                continue;
            };
            let sum: f32 = brain.gate_terms.iter().map(|term| term.contribution).sum();
            assert!(
                (sum - brain.gate).abs() < 1e-4,
                "step {step}: the terms add to {sum} but the gate is {}",
                brain.gate
            );
            let raw: f32 = brain
                .gate_terms
                .iter()
                .filter(|term| term.key != "clamp")
                .map(|term| term.contribution)
                .sum();
            if raw > brain.gate + 1e-4 {
                clamped_high = true;
            }
            if raw < brain.gate - 1e-4 {
                clamped_low = true;
            }
        }
        assert!(
            clamped_high,
            "the test never reached a gate clamped at the top, so the upper \
             clamp was never checked"
        );
        assert!(
            clamped_low,
            "the test never reached a gate clamped at the bottom, so the lower \
             clamp was never checked"
        );
    }

    #[test]
    fn the_gate_still_teaches_nothing_when_shut_at_any_rate() {
        // The learning rate is a multiplier on how large a permitted update is,
        // not a fourth factor of the rule. Turning it up must not turn a shut
        // modulator into a way of learning, or the three-factor rule becomes
        // decoration.
        for rate in [0.5, 1.0, 20.0] {
            let mut fly = tfly::Fly::new();
            fly.seed(7);
            fly.set_learn_rate(rate);
            for _ in 0..500 {
                let _ = fly.train_trial_gated(
                    &test_lesson(),
                    0.0,
                    &[tfly::action::REST, tfly::action::DANCE, tfly::action::EAT],
                    Some(0.0),
                );
            }
            assert_eq!(
                fly.weight(tfly::cue::LIGHT, tfly::action::DANCE),
                0.0,
                "a shut gate taught something at rate {rate}"
            );
        }
    }

    #[test]
    fn a_faster_rate_reaches_the_same_lesson_sooner() {
        // The point of exposing the rate: the same lesson, taken in fewer
        // trials, without changing what is finally learnt.
        let reach = |rate: f32| {
            let mut fly = tfly::Fly::new();
            fly.seed(11);
            fly.set_learn_rate(rate);
            let mut trials = 0;
            while trials < 20_000 && fly.weight(tfly::cue::LIGHT, tfly::action::DANCE) < 0.9 {
                let _ = fly.train_trial_gated(
                    &test_lesson(),
                    0.1,
                    &[tfly::action::REST, tfly::action::DANCE, tfly::action::EAT],
                    Some(1.0),
                );
                trials += 1;
            }
            trials
        };
        let slow = reach(1.0);
        let fast = reach(8.0);
        assert!(slow > 100, "the default rate is not the slow one: {slow}");
        assert!(
            fast < slow,
            "rate 8 took {fast} trials against {slow} at rate 1"
        );
    }

    #[test]
    fn the_panel_states_the_association_it_is_training() {
        // "What is she learning on" was unanswerable from a percentage and a
        // mood. The association itself has to be in the panel, in the same words
        // the act list uses.
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"ruins"}"#)
            .expect("act");
        let brain = runtime.brain_snapshot().expect("a brain");
        let act = ACTS.iter().find(|a| a.id == "ruins").expect("the act");
        assert_eq!(
            brain.learning,
            format!("{} → {}", cue_name(act.cue), action_name(act.solution))
        );
    }

    #[test]
    fn the_panel_reads_russian_rather_than_core_keys() {
        // The core names an emotion and a drive in ASCII, which is right for
        // code and unreadable in a panel. The panel has to show the Russian
        // names, or a reader sees "настроение joy" and learns nothing from it.
        let mut runtime = EditorRuntime::new(1);
        for _ in 0..240 {
            runtime.step(FIXED_DT);
        }
        let brain = runtime.brain_snapshot().expect("a brain");
        for (label, value) in [("mood", &brain.mood), ("drive", &brain.drive)] {
            assert!(
                value
                    .chars()
                    .all(|c| 0x0400 <= c as u32 && c as u32 <= 0x04ff
                        || c.is_whitespace()
                        || c == '?'),
                "the {label} is not Russian: {value:?}"
            );
        }
    }

    #[test]
    fn the_last_outcome_says_what_the_trial_was() {
        // These three strings were mapped onto the outcomes in the wrong order,
        // so a wrong answer reported itself as "почти" and a correct one as
        // "освоила «...»", which is the one that reads as a claim of success.
        // A panel that mislabels its own result is worse than a silent one.
        let label = |outcome: tfly::TrialOutcome| match outcome {
            tfly::TrialOutcome::Correct => "верно",
            tfly::TrialOutcome::Partial => "почти",
            tfly::TrialOutcome::Wrong => "ошибка",
        };
        assert_ne!(
            label(tfly::TrialOutcome::Correct),
            label(tfly::TrialOutcome::Wrong)
        );
        assert_ne!(
            label(tfly::TrialOutcome::Correct),
            label(tfly::TrialOutcome::Partial)
        );
        assert_ne!(
            label(tfly::TrialOutcome::Wrong),
            label(tfly::TrialOutcome::Partial)
        );
        assert_eq!(label(tfly::TrialOutcome::Correct), "верно");
        assert_eq!(label(tfly::TrialOutcome::Wrong), "ошибка");

        // And the recorded value has to agree with the core's own outcome codes,
        // which is checked by running a trial whose answer is forced.
        let mut fly = tfly::Fly::new();
        fly.seed(3);
        let result = fly.train_trial_gated(
            &test_lesson(),
            0.0,
            &[tfly::action::DANCE, tfly::action::REST],
            Some(1.0),
        );
        assert_eq!(result.outcome, tfly::TrialOutcome::Correct);
        assert_eq!(label(result.outcome), "верно");
    }

    #[test]
    fn the_learn_rate_reaches_every_fly_including_new_ones() {
        let mut runtime = EditorRuntime::new(2);
        runtime
            .apply_command(br#"{"action":"learnrate","value":4}"#)
            .expect("learnrate");
        for brain in &runtime.brains {
            assert_eq!(brain.fly.learn_rate(), 4.0, "an existing fly missed it");
        }
        runtime.apply_command(br#"{"action":"add"}"#).expect("add");
        let last = runtime.brains.last().expect("a new brain");
        assert_eq!(
            last.fly.learn_rate(),
            4.0,
            "a new character joined at the default pace"
        );
        let brain = runtime.brain_snapshot().expect("a brain");
        assert_eq!(brain.learn_rate, 4.0);
    }

    #[test]
    fn a_character_stands_on_the_stage_rather_than_in_it() {
        // A stage is a raised disc. Placed at a flat zero, a character walking
        // onto one stood ankle-deep inside it, which is what a stage is for
        // avoiding.
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"village"}"#)
            .expect("act");
        let stage = ACTS
            .iter()
            .find(|a| a.id == "village")
            .expect("the village district exists");
        assert!(
            stage.stage_height > 0.0,
            "a stage has to be raised or there is nothing to stand on"
        );
        // Drop her in the middle of it and let the height settle.
        runtime.agents[0].position[0] = stage.origin[0];
        runtime.agents[0].position[1] = stage.origin[1];
        for _ in 0..240 {
            runtime.step(FIXED_DT);
        }
        let on_stage = runtime.snapshot(60.0).agents[0].ground;
        assert!(
            (on_stage - stage.stage_height).abs() < 0.01,
            "she must stand on the stage top at {}, got {on_stage}",
            stage.stage_height
        );

        // And step down to the tent floor when she leaves it.
        //
        // One step, and that is the point. She has a course now and the food is
        // on her stage, so she walks back onto it: waiting for the height to settle
        // used to be enough and now is not, because waiting is time enough to be
        // carried back. What is checked here is the surface under her feet, and
        // that answers in one step.
        runtime.agents[0].position[0] = stage.origin[0] + stage.stage_radius + 0.8;
        runtime.step(FIXED_DT);
        let leaving = runtime.snapshot(60.0).agents[0].ground;
        assert!(
            leaving < stage.stage_height,
            "the floor has to start dropping as she leaves the stage, got {leaving}"
        );
        assert!(
            leaving > 0.0,
            "and it is eased, not dropped in one step, got {leaving}"
        );
    }

    #[test]
    fn every_stage_has_room_for_a_character_to_mill_around() {
        // Characters circle their stage so a troupe spreads out instead of
        // stacking. If that circle is bigger than the clear floor, they grind
        // against the scenery with the stage still pulling them in. The
        // labyrinth used to fail this badly: its walls left a clear centre of
        // 0.39 on a 1.7 stage, which is not a floor.
        for act in ACTS.iter() {
            let room = orbit_room(act);
            // The widest circle any of the three flies will mill at, which is
            // the third fly's unclamped want after the clamp.
            let widest = (0.55f32 + 0.18f32 * 3.0f32).min(room);
            assert!(room > 0.0, "{} leaves no room at all to stand in", act.id);
            assert!(
                widest + BODY_RADIUS <= clear_centre(act) + 1e-4,
                "{}: a character milling at {widest} reaches {clear} from the centre, \
                 which is inside the scenery",
                act.id,
                clear = clear_centre(act)
            );
        }
    }

    #[test]
    fn a_character_cannot_walk_through_a_wall() {
        // The scenery and the colliders come from the same numbers, so this is
        // checking that the resolution actually works rather than that the two
        // lists agree.
        //
        // It measures against the course, because that is what she is walking into
        // now. The labyrinth's old ring of walls is scenery no more, and testing
        // against it would pass without her ever meeting it.
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"village"}"#)
            .expect("act");
        let stage = ACTS
            .iter()
            .find(|a| a.id == "village")
            .expect("the village district exists");
        let wall = runtime.course_solids[1]
            .iter()
            .copied()
            .find(|c| c[2] > 0.0)
            .expect("the course has walls to walk into");
        assert!(
            wall[0].abs() < stage.stage_radius + stage.origin[0].abs(),
            "the wall has to be on the stage to be worth testing"
        );

        // Start her well past the wall and let the steering pull her back.
        runtime.agents[0].position[0] = wall[0] * 3.0;
        runtime.agents[0].position[1] = wall[1] * 3.0;
        let mut worst = 0.0f32;
        for _ in 0..3000 {
            runtime.step(FIXED_DT);
            let a = &runtime.agents[0];
            let dx = a.position[0] - wall[0];
            let dy = a.position[1] - wall[1];
            let depth = wall[2] + BODY_RADIUS - (dx * dx + dy * dy).sqrt();
            if depth > worst {
                worst = depth;
            }
        }
        assert!(
            worst < 0.12,
            "she must not end up inside a wall, got {worst} past the keep-out line"
        );
    }

    #[test]
    fn the_walk_phase_follows_the_ground() {
        // The phase used to run on its own clock, so the legs cycled several
        // times faster than the body moved and the feet skated. The world is
        // the only thing that can say how fast she is really going.
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"hill"}"#)
            .expect("act");
        for _ in 0..120 {
            runtime.step(FIXED_DT);
        }
        let first = runtime.agents[0].gait_phase;
        let first_steps = runtime.brains[0].fly.step_count();
        for _ in 0..300 {
            runtime.step(FIXED_DT);
        }
        let advanced = runtime.agents[0].gait_phase - first;
        let took = runtime.brains[0].fly.step_count() - first_steps;
        assert!(
            advanced > 0.5,
            "she has to be walking for the phase to mean anything, got {advanced}"
        );
        assert!(
            took > 0.0,
            "and the ground speed has to reach the model, or the feet skate"
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

    /// How many seconds of simulation a character is given to find the food.
    const COURSE_BUDGET: f32 = 240.0;

    #[test]
    fn somebody_finds_food_in_a_maze_the_size_of_a_village() {
        // The point of the whole thing. A maze nobody can solve is a hedge, and a
        // score that only goes up when a character happens to walk over a drifting
        // target point is what this replaced.
        //
        // Pointed at a maze district. The hill, which is where a fly wakes up, is
        // open ground and there is nothing to search there.
        //
        // It asks whether she gets *some* food, not whether she finishes. In a
        // maze of about a hundred rooms she does not finish in a round, and
        // measuring it showed she does not finish in five minutes either. What she
        // does is get a long way in, and how far is the score the panel shows.
        let mut runtime = EditorRuntime::new(3);
        runtime
            .apply_command(br#"{"action":"act","name":"village"}"#)
            .expect("act");
        let course = runtime.snapshot(60.0).shown_course().clone();
        assert!(
            course.feeds.len() >= 3,
            "a village this size has one prize?"
        );
        assert!(
            course.cell * 11.0 > 7.0,
            "the village maze came out only {:.1} across",
            course.cell * 11.0
        );
        let mut elapsed = 0.0f32;
        let mut best = 0u32;
        while elapsed < COURSE_BUDGET {
            runtime.step(FIXED_DT);
            elapsed += FIXED_DT;
            let state = runtime.snapshot(60.0);
            for runner in state.shown_course().runners.iter() {
                best = best.max(runner.ate_count);
            }
            if best > 0 {
                break;
            }
        }
        assert!(
            best > 0,
            "nobody found so much as one mouthful in {COURSE_BUDGET} s"
        );
    }

    #[test]
    fn the_hill_is_open_ground_and_the_food_is_still_reached() {
        // The hill is where a fly wakes up: a tree and open ground, and no maze.
        // A newborn has to be able to walk to the food without a single wall to
        // guide her, which is the first thing she has to learn and the one thing a
        // maze cannot teach.
        let runtime = EditorRuntime::new(1);
        let course = runtime.snapshot(60.0).shown_course().clone();
        assert!(
            course.walls.is_empty(),
            "the hill grew a maze, which is the one thing it must not have"
        );
        let mut runtime = runtime;
        let mut elapsed = 0.0f32;
        let mut fed = false;
        while elapsed < COURSE_BUDGET && !fed {
            runtime.step(FIXED_DT);
            elapsed += FIXED_DT;
            fed = runtime.snapshot(60.0).shown_course().fed > 0;
        }
        assert!(fed, "she never walked to the food across open ground");
    }

    #[test]
    fn a_character_reaches_the_food_on_a_course_she_can_walk() {
        // A single character has nobody to follow a trail of, so this is the
        // honest test of the searching on its own: the smell pulls her across the
        // floor and the wall commitment gets her round the obstacles.
        for seed in [7u32, 101, 2024] {
            let mut runtime = EditorRuntime::new(1);
            runtime
                .apply_command(format!(r#"{{"action":"course","value":{seed}}}"#).as_bytes())
                .expect("course");
            let mut elapsed = 0.0f32;
            let mut ate = false;
            while elapsed < COURSE_BUDGET && !ate {
                runtime.step(FIXED_DT);
                elapsed += FIXED_DT;
                ate = runtime.snapshot(60.0).shown_course().runners[0].fed;
            }
            assert!(ate, "seed {seed}: she never found the food");
        }
    }

    #[test]
    fn the_maze_districts_have_a_maze_each_and_the_hill_does_not() {
        // The hill is where a fly wakes up, and the one thing she is not on the
        // morning she wakes is lost. So the hill is open ground with a tree, and
        // every other district is a maze.
        //
        // All five are on screen at once, so a maze on one and bare floor on the
        // others would read as a bug. The test states which is which rather than
        // pretending they are the same.
        let state = EditorRuntime::new(3).snapshot(60.0);
        assert_eq!(state.courses.len(), ACTS.len());
        let mut mazes: Vec<Vec<WallSnapshot>> = Vec::new();
        for (index, course) in state.courses.iter().enumerate() {
            let act = &ACTS[index];
            assert_eq!(
                course.act, index,
                "a course is filed under the wrong district"
            );
            assert_eq!(
                course.walls.is_empty(),
                act.maze_cells == 0,
                "district {index} ({}) does not match its own maze size",
                act.id
            );
            if act.maze_cells == 0 {
                continue;
            }
            assert!(
                !course.walls.is_empty(),
                "district {index} has nothing to search"
            );
            assert!(course.path_length > 0.0, "district {index} has no route");
            assert!(
                course.cell * act.maze_cells as f32 > 6.0,
                "district {index} wants a {}-cell maze but came out {:.1} across",
                act.maze_cells,
                course.cell * act.maze_cells as f32
            );
            mazes.push(course.walls.clone());
        }
        assert!(mazes.len() >= 3, "the world has fewer mazes than it should");
        for (i, first) in mazes.iter().enumerate() {
            for other in &mazes[i + 1..] {
                assert_ne!(first, other, "two districts were handed the same maze");
            }
        }
    }

    #[test]
    fn the_course_is_rebuilt_and_the_route_is_not_reusable() {
        // Rebuilding is the whole reason a round is a round. If the same maze came
        // back, a character could learn it once and never look again, and the
        // thing on screen would be a rehearsal.
        let mut runtime = EditorRuntime::new(1);
        runtime
            .apply_command(br#"{"action":"act","name":"village"}"#)
            .expect("act");
        let first = runtime.snapshot(60.0).shown_course().clone();
        runtime
            .apply_command(br#"{"action":"course","value":987654}"#)
            .expect("course");
        let second = runtime.snapshot(60.0).shown_course().clone();
        assert_ne!(first.walls.len(), 0, "the village has no maze to rebuild");
        assert_ne!(
            first.walls, second.walls,
            "a new seed produced the same maze"
        );
    }

    #[test]
    fn everyone_starts_at_the_entrance_and_nobody_starts_inside_a_wall() {
        let runtime = EditorRuntime::new(3);
        let course = runtime.snapshot(60.0).shown_course().clone();
        assert_eq!(course.count, 3);
        // The food has to be a walk away, not a stride. The route is the honest
        // measure and it is what the panel shows.
        assert!(
            course.path_length > course.cell * 3.0,
            "the course is a walk of {:.2}, too short to be one",
            course.path_length
        );
        for runner in &course.runners {
            assert!(runner.distance > 0.0, "a character started on the food");
        }
    }

    #[test]
    fn a_character_never_ends_a_frame_inside_a_wall() {
        // The collision lives in the body and runs immediately after the move. With
        // a maze in the way, "immediately after" is the difference between walking
        // round a wall and being inside one every few frames.
        let mut runtime = EditorRuntime::new(3);
        let mut worst = 0.0f32;
        for _ in 0..1200 {
            runtime.step(FIXED_DT);
            for agent in &runtime.agents {
                for solid in &agent.solids {
                    if solid[2] <= 0.0 {
                        continue;
                    }
                    let dx = agent.position[0] - solid[0];
                    let dz = agent.position[1] - solid[1];
                    let overlap = solid[2] + BODY_RADIUS - dx.hypot(dz);
                    worst = worst.max(overlap);
                }
            }
        }
        assert!(
            worst < 0.02,
            "a character was {worst:.4} inside a wall at the end of a frame"
        );
    }

    #[test]
    fn reaching_the_food_rewards_the_brain_and_not_only_a_counter() {
        // The reward goes into the character, so the one that finds the food looks
        // like it: the pulse is what the face and the posture are reading, and a
        // counter in the snapshot would look like nothing at all.
        let mut runtime = EditorRuntime::new(1);
        let mut before = None;
        let mut elapsed = 0.0f32;
        let mut rewarded = false;
        while elapsed < COURSE_BUDGET {
            runtime.step(FIXED_DT);
            elapsed += FIXED_DT;
            if before.is_none() {
                before = Some(runtime.snapshot(60.0).agents[0].reward);
            }
            if runtime.snapshot(60.0).shown_course().runners[0].fed {
                rewarded = true;
                break;
            }
        }
        assert!(rewarded, "she never found the food to be rewarded for");
        assert_eq!(runtime.snapshot(60.0).agents[0].reward, 1.0);
        let _ = before;
    }

    #[test]
    fn a_round_ends_and_the_next_one_is_a_new_problem() {
        let mut runtime = EditorRuntime::new(2);
        let first = runtime.snapshot(60.0).shown_course().round;
        // Force the round out rather than waiting out the clock.
        runtime.round_time = 0.001;
        runtime.step(FIXED_DT);
        let after = runtime.snapshot(60.0).shown_course().clone();
        assert!(after.round > first, "the round did not advance");
        assert!(after.time_left > 1.0, "the next round started with no time");
        assert!(
            after.runners.iter().all(|r| !r.fed),
            "somebody is still fed"
        );
    }

    #[test]
    fn the_round_log_says_how_the_round_went() {
        let mut runtime = EditorRuntime::new(1);
        runtime.round_time = 0.001;
        runtime.step(FIXED_DT);
        let state = runtime.snapshot(60.0);
        let texts: Vec<&str> = state.training.log.iter().map(|e| e.text.as_str()).collect();
        assert!(
            texts.iter().any(|t| t.contains("раунд")),
            "nobody said how the round went: {texts:?}"
        );
    }
}
