//! The life: chapters, a household, and sleep.
//!
//! This is where the profile, the six tasks and the fly's own body meet. Three
//! things live here, and they are separate on purpose:
//!
//! - [`Life`] owns which chapter she is in and what has to happen to leave it.
//! - [`Family`] is the household she was born into, because the life the project
//!   was asked for starts in a simple peasant family and not on a stage.
//! - [`Dream`] is what a sleep is: the body lies still and the same brain is put
//!   into one of the six tasks from [`crate::task`].
//!
//! ## The sleep is the training, and it is not a metaphor
//!
//! [`Dream::step`] does the following, once per turn:
//!
//! 1. the task describes itself to her in the four senses she has;
//! 2. those four numbers are quantised into a cue from the C core's own
//!    vocabulary of twelve, because that is the resolution her brain actually
//!    has;
//! 3. she picks an action, mostly the one she has associated with this cue
//!    before and sometimes a random one, which is the same epsilon-greedy rule
//!    the rest of the project uses;
//! 4. she performs it, which is what fills the eligibility trace in the C core
//!    and without which nothing can be learned at all;
//! 5. the task scores the turn, and that score is written into the association
//!    with the learning gate as the modulator.
//!
//! So the thing that is learned is a real weight in the same table the fly uses
//! to walk. It is not a number in a file that goes up, and it is not the task
//! telling her the answer: the tasks in [`crate::task`] never say what the right
//! action is, they only say how the turn went. That is why melody and code work
//! at all here, and it is why the sixth task is the sixth.
//!
//! What "перенос сознания в обучение" is, in this code: while the body is
//! asleep and on screen shows her lying still, the policy is being trained in
//! another room. Nothing more mystical than that, and nothing less.

use crate::profile::{Profile, Stage, Task};
use crate::task::{Episode, Senses};
use crate::tfly::{self, Fly};

/// How long a newborn lies before she opens her eyes.
///
/// Sim seconds, and deliberately short. A chapter that takes minutes of real
/// time is a chapter nobody watches, and the whole point of this life is that
/// somebody can sit and look at it happen.
pub const SWADDLE_SECONDS: f32 = 18.0;

/// How far she has to walk before anyone calls it walking.
pub const WALK_THRESHOLD: f32 = 0.3;

/// How far from home counts as lost, in world units.
///
/// The village is thirteen units from the middle of the world and the districts
/// are thirteen apart, so this is about one district: far enough that she cannot
/// see the house, near enough that getting back is a walk rather than a journey.
pub const LOST_DISTANCE: f32 = 11.0;

/// How close to home counts as arrived.
pub const HOME_DISTANCE: f32 = 4.0;

/// The four senses, folded into one of the twelve cues the brain has.
///
/// ## Why a number in 0..1 has to become a named cue
///
/// The C core associates a *cue* with an *action*, and there are twelve cues:
/// `LIGHT`, `DARK`, `ODOR_FRUIT`, `ODOR_FLOWER`, `ODOR_MALE`, `ODOR_TOUCH`,
/// `TEMPERATURE`, `SOUND`, `VIBRATION`, `VISUAL`, `GRAVITY`. There is no
/// continuous channel, because a fly does not have one: what she has is "bright",
/// "fruit", "something touching me", and that is a category, not a number.
///
/// So the strongest channel decides, and the others are discarded. That sounds
/// like throwing information away, and it is, and it is also the only honest
/// option: a brain that could read a float would not be this brain, and skills
/// learned in a richer representation would not be the skills she walks with.
pub fn cue_of(senses: &Senses) -> i32 {
    let light = senses.light.clamp(0.0, 1.0);
    let odor = senses.odor.clamp(0.0, 1.0);
    let touch = senses.touch.clamp(0.0, 1.0);
    let temperature = senses.temperature.clamp(0.0, 1.0);

    // Nothing at all is its own state, and it has to be tested first. The first
    // version built "darkness" as a channel of `1 - light` and then took the
    // maximum, which meant a pitch-dark room scored a perfect darkness and beat
    // every other sense: a fly in an unlit room with her hand on a wall was
    // reported as being in the dark rather than as touching something. A test
    // with a dark room and a touch in it caught that, and a second one caught the
    // empty room it left behind.
    if [light, odor, touch, temperature].iter().all(|v| *v < 0.1) {
        return tfly::cue::VIBRATION;
    }

    // The strongest channel decides, and there is no inverted channel competing
    // with the other three.
    let strongest = [
        (light, tfly::cue::LIGHT),
        (odor, tfly::cue::ODOR_FRUIT),
        (touch, tfly::cue::TOUCH),
        (temperature, tfly::cue::TEMPERATURE),
    ]
    .into_iter()
    .fold((0.0f32, tfly::cue::VIBRATION), |best, channel| {
        if channel.0 > best.0 { channel } else { best }
    });

    // Darkness is not a separate sense. It is the light channel being the one she
    // can feel and there being very little of it, which is a different statement
    // from "she is feeling something else in a dark room".
    if strongest.1 == tfly::cue::LIGHT && strongest.0 < 0.3 {
        return tfly::cue::DARK;
    }
    strongest.1
}

/// The fly's own action vocabulary, in the order the tasks index them.
///
/// Ten actions, ten motors. A task with five choices uses the first five, a task
/// with nine uses the first nine. Deliberately the same list for every task: a
/// fly that solved Tetris with `DANCE` and the maze with `EAT` would be learning
/// two unrelated things, and the transfer between the six is the whole claim.
const MOTORS: [i32; 9] = [
    tfly::action::FORWARD,
    tfly::action::TURN_L,
    tfly::action::TURN_R,
    tfly::action::UP,
    tfly::action::DOWN,
    tfly::action::FLAP,
    tfly::action::DANCE,
    tfly::action::EAT,
    tfly::action::MATE,
];

/// The motor a task's action index means.
#[must_use]
pub fn motor(index: usize) -> i32 {
    MOTORS[index.min(MOTORS.len() - 1)]
}

/// One sleep, in one task.
#[derive(Debug)]
pub struct Dream {
    /// The task she is in.
    pub episode: Episode,
    /// The cue she saw when she made her last choice, kept so the association
    /// can be written *after* she has acted. The eligibility trace in the C core
    /// is filled by acting, so an association written before the action is an
    /// association about a decision she had not made yet.
    cue: i32,
    /// The motor she used.
    motor: i32,
    /// How many turns the sleep has run.
    pub turns: u32,
    /// How much reward the sleep has earned so far, for the panel.
    pub reward: f32,
    /// How many turns she got right, out of the turns she has taken.
    pub good_turns: u32,
}

impl Dream {
    /// A sleep in a task, at the level she has reached.
    pub fn new(task: Task, level: f32, seed: u32, sleeps: u32) -> Dream {
        Dream {
            episode: Episode::new(task, level, seed, sleeps + 1),
            cue: tfly::cue::VIBRATION,
            motor: tfly::action::REST,
            turns: 0,
            reward: 0.0,
            good_turns: 0,
        }
    }

    /// Which task this sleep is in.
    pub fn task(&self) -> Task {
        self.episode.task()
    }

    /// Whether the sleep is over.
    pub fn done(&self) -> bool {
        self.episode.over()
    }

    /// Take one turn of the sleep: perceive, choose, act, be scored, learn.
    ///
    /// Returns the reward for the turn. The four steps are in this order for a
    /// reason: the choice has to be made from what she can see *now*, the
    /// eligibility trace has to be filled by the action, and only then can the
    /// association be written with the modulator that follows from how the turn
    /// went.
    pub fn step(&mut self, fly: &mut Fly, epsilon: f32) -> f32 {
        if self.done() {
            return 0.0;
        }
        let senses = self.episode.observe();
        self.cue = cue_of(&senses);
        let space = self.episode.actions();

        // Epsilon-greedy over the associations in the C core, exactly as the
        // editor's own trials do it. Mostly she does what she has learned this
        // cue leads to; sometimes she does not, and without that she would try
        // each task's right answer once and never find it again.
        let chosen = if fly.random() < epsilon {
            fly.random_index(space)
        } else {
            let mut best = 0usize;
            let mut best_w = fly.weight(self.cue, motor(0));
            for index in 1..space {
                let w = fly.weight(self.cue, motor(index));
                if w > best_w {
                    best = index;
                    best_w = w;
                }
            }
            best
        };
        self.motor = motor(chosen);

        // Let her actually do it. A harness that scores a fly without letting it
        // act cannot teach it anything, and the comment in `train_trial` in the
        // editor says the same about the trials that came before this.
        fly.clear_actions();
        fly.act(self.motor, 1.0);
        // Six steps, not two, because the eligibility trace in the C core is built
        // as `action * dt` and decays each step. Two steps of a sixtieth of a
        // second put about one sixtieth into the trace, and a write-back scaled by
        // that lands under a millionth, which is to say nowhere. Six steps is what
        // the editor's own trials use, and it is the difference between learning
        // and not.
        fly.steps(6, 1.0 / 60.0);

        let gained = self.episode.act(chosen);
        self.turns += 1;
        self.reward += gained;
        if gained > 0.0 {
            self.good_turns += 1;
        }

        // The write-back, in the shape the model was built for: the turn is
        // delivered as an unconditioned stimulus, which is what opens the
        // dopamine that opens the learning gate, and the association is then
        // written with that gate as its modulator.
        //
        // The first version skipped the stimulus and folded the size of the turn
        // into the modulator, so the gate was multiplied by the reward twice over
        // and by a trace that was one sixtieth full. The result was a write-back of
        // about two millionths, which reads as exactly zero over a whole sleep, and
        // the skill bar went up anyway because it was computed from the task's
        // score rather than from the fly. A fly whose brain never changed and a fly
        // that had learnt something were showing the same number, and nothing on
        // screen could tell them apart.
        if gained > 0.0 {
            fly.reward((gained * 4.0).clamp(0.0, 1.0));
        } else if gained < 0.0 {
            fly.punish((-gained * 4.0).clamp(0.0, 1.0));
        }
        let gate = fly.learning_gate();
        if gate > 0.001 && gained != 0.0 {
            fly.associate(self.cue, self.motor, gained.signum(), gate);
        }
        gained
    }

    /// What she has got out of this sleep, as a level gain.
    ///
    /// Scaled so a whole sleep is worth something visible and a bad sleep is
    /// worth nothing rather than a loss. A fly who slept badly did not get worse
    /// at something; she just did not get better, and a bar that can go down on
    /// a bad night is a bar that teaches a player to avoid sleeping.
    pub fn gain(&self) -> f32 {
        if self.turns == 0 {
            return 0.0;
        }
        // Reward *per turn*, not reward in total.
        //
        // Total reward rewards lasting, and lasting is not skill: a policy that
        // survives by taking the safest action available scores the same as one
        // that solves the task, and a policy that flails for a hundred turns in a
        // maze outscores one that finds the exit in twelve. Per turn asks the
        // question that matters, which is how good she was.
        //
        // The reward for solving is added whole rather than scaled, because
        // finishing is worth a discontinuity: a fly that solves it occasionally
        // should be preferred to one that is reliably mediocre, and a linear
        // per-turn average does not say that on its own.
        let per_turn = (self.reward / self.turns as f32).clamp(0.0, 0.12);
        let finished = if self.episode.solved() { 0.08 } else { 0.0 };
        (per_turn + finished).clamp(0.0, 0.25)
    }

    /// One line about where she is, for the on-screen dream.
    pub fn describe(&self) -> String {
        self.episode.describe()
    }
}

/// One person in the household.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// Their name, in Russian.
    pub name: &'static str,
    /// What they are to her.
    pub role: &'static str,
    /// Roughly how old, in years, for the readout.
    pub age: u32,
    /// What they are doing just now, in Russian.
    pub doing: &'static str,
}

/// The household she was born into.
///
/// Four people and no more. A simple peasant family, which is what was asked
/// for, and a deliberately short list: this is scenery that gives the life
/// somewhere to be, not a second set of characters with their own needs. They do
/// not walk around, they do not need feeding, and they are not simulated.
#[derive(Debug, Clone, PartialEq)]
pub struct Family {
    pub members: Vec<Member>,
    /// Where the house is, in world units. The village is at thirteen on x.
    pub home: [f32; 3],
    /// The name of the village, in Russian.
    pub place: &'static str,
}

impl Family {
    /// A simple household in the village.
    pub fn new() -> Family {
        Family {
            members: vec![
                Member {
                    name: "Равна",
                    role: "мать",
                    age: 34,
                    doing: "топит печь",
                },
                Member {
                    name: "Осип",
                    role: "отец",
                    age: 39,
                    doing: "идёт с поля",
                },
                Member {
                    name: "Тина",
                    role: "бабушка",
                    age: 68,
                    doing: "прядет",
                },
                Member {
                    name: "Кузьма",
                    role: "брат",
                    age: 7,
                    doing: "играет у двери",
                },
            ],
            home: [13.0, 0.0, 1.4],
            place: "деревня",
        }
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the household is empty, which it never is.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// A one-line summary for the panel.
    pub fn summary(&self) -> String {
        let who: Vec<String> = self
            .members
            .iter()
            .map(|m| format!("{} — {}", m.name, m.role))
            .collect();
        format!("семья из {}: {}", self.members.len(), who.join(", "))
    }

    /// Whether she is within talking distance of the house.
    pub fn is_home(&self, position: [f32; 3]) -> bool {
        distance(position, self.home) <= HOME_DISTANCE
    }

    /// Whether she is far enough to be lost.
    pub fn is_away(&self, position: [f32; 3]) -> bool {
        distance(position, self.home) > LOST_DISTANCE
    }
}

impl Default for Family {
    fn default() -> Self {
        Family::new()
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// What she is trying to do, and how far along she is.
#[derive(Debug, Clone, PartialEq)]
pub struct Quest {
    /// In Russian, one line, for the panel.
    pub text: &'static str,
    /// 0..1.
    pub progress: f32,
    /// Whether it is finished and the next chapter can open.
    pub done: bool,
    /// What she would be doing instead, for the panel when the quest is done.
    pub after: &'static str,
}

impl Quest {
    fn new(text: &'static str, after: &'static str) -> Quest {
        Quest {
            text,
            progress: 0.0,
            done: false,
            after,
        }
    }
}

/// The life.
#[derive(Debug)]
pub struct Life {
    pub profile: Profile,
    pub family: Family,
    pub quest: Quest,
    /// The sleep she is having, if she is asleep.
    pub dream: Option<Dream>,
    /// Sim seconds since she lay down.
    pub asleep_for: f32,
    /// The task she will sleep into next.
    pub task: Task,
    /// Every sleep she has had, newest last, for the readout.
    pub history: Vec<SleepRecord>,
    /// Sim seconds spent lying in swaddling.
    pub swaddled_for: f32,
    /// The chapter she left and why, for the log.
    pub last_change: Option<String>,
    /// Set when something worth saving happened, so the runtime knows to write.
    ///
    /// A sleep that ends on its own has to mark this. The first version saved
    /// only when a person pressed "разбудить", so a sleep that finished while
    /// nobody was watching — which is every sleep, because they end in about two
    /// seconds — was never written down, and the profile on disk still said she
    /// had never slept. The number went up on screen and nowhere else.
    pub dirty: bool,
}

/// One finished sleep.
#[derive(Debug, Clone, PartialEq)]
pub struct SleepRecord {
    pub task: Task,
    pub reward: f32,
    pub turns: u32,
    pub good_turns: u32,
    pub score: u32,
    pub solved: bool,
    /// Her level in this task afterwards.
    pub level: f32,
}

impl Life {
    /// A newborn's life, from a profile.
    pub fn new(profile: Profile) -> Life {
        let task = task_for(Stage::Swaddled);
        Life {
            family: Family::new(),
            quest: Quest::new("лежит в пелёнках", "открывает глаза"),
            task,
            dream: None,
            asleep_for: 0.0,
            history: Vec::new(),
            swaddled_for: 0.0,
            last_change: None,
            dirty: false,
            profile,
        }
    }

    /// The chapter she is in.
    pub fn stage(&self) -> Stage {
        self.profile.stage
    }

    /// Whether she is asleep.
    pub fn is_asleep(&self) -> bool {
        self.dream.is_some()
    }

    /// Put her to sleep. She begins the next task her chapter calls for.
    ///
    /// The level she starts at is the level she has actually reached, which is
    /// what makes the six tasks a curriculum rather than six things to do once.
    pub fn sleep(&mut self, fly: &mut Fly, seed: u32) {
        if self.is_asleep() {
            return;
        }
        let task = self.task;
        let level = self.profile.skill(task).level;
        self.profile.sleeping_into = Some(task);
        self.dream = Some(Dream::new(task, level, seed, self.profile.sleeps));
        self.asleep_for = 0.0;
        let _ = fly;
    }

    /// Advance the sleep by one turn.
    pub fn dream_step(&mut self, fly: &mut Fly, epsilon: f32) -> f32 {
        let turn = match &mut self.dream {
            Some(dream) => {
                let gained = dream.step(fly, epsilon);
                let _ = gained;
                if dream.done() {
                    Some(dream.task())
                } else {
                    None
                }
            }
            None => None,
        };
        if let Some(task) = turn {
            self.wake(fly, task);
        }
        self.asleep_for += crate::editor::FIXED_DT;
        // The turn's reward is already banked in the dream and, on the last
        // turn, in the profile. There is nothing for the caller of `tick` to do
        // with it, so it is not returned; the panel reads the history.
        0.0
    }

    /// Wake her up and write down what the sleep was worth.
    pub fn wake(&mut self, fly: &mut Fly, task: Task) {
        let dream = match self.dream.take() {
            Some(dream) => dream,
            None => return,
        };
        let before = self.profile.skill(task).level;
        let gain = dream.gain();
        self.profile.train(task, gain, dream.episode.score());
        let after = self.profile.skill(task);
        self.profile.sleeps = self.profile.sleeps.saturating_add(1);
        self.profile.sleeping_into = None;
        self.profile.saved_at = self.profile.age;
        self.history.push(SleepRecord {
            task,
            reward: dream.reward,
            turns: dream.turns,
            good_turns: dream.good_turns,
            score: dream.episode.score(),
            solved: dream.episode.solved(),
            level: after.level,
        });
        self.last_change = Some(format!(
            "{}: {} -> {}",
            task.name(),
            trim(before),
            trim(after.level)
        ));
        self.dirty = true;
        let _ = fly;
    }

    /// Move the life on: one tick of the world, and the chapter gates with it.
    ///
    /// `ate` is how many foods she has eaten in the world this round, which is
    /// the world's own measure of whether she can feed herself. `position` is
    /// where she is standing, which is the only thing the later chapters can
    /// possibly be about.
    pub fn tick(&mut self, fly: &mut Fly, dt: f32, position: [f32; 3], ate: u32, epsilon: f32) {
        self.profile.age += dt;
        // A newborn's eyes open whether she is asleep or not. The first version
        // counted the swaddling time only while awake, and a fly who spent her
        // first ten lives asleep stayed in chapter one for ever — which is the
        // opposite of what sleep is for at that age, and it meant the whole
        // curriculum behind her was unreachable.
        if self.profile.stage == Stage::Swaddled {
            self.swaddled_for += dt;
        }
        if self.is_asleep() {
            self.dream_step(fly, epsilon);
            if self.profile.stage == Stage::Swaddled {
                self.quest.progress = (self.swaddled_for / SWADDLE_SECONDS).min(1.0);
                if self.swaddled_for >= SWADDLE_SECONDS {
                    self.enter(Stage::FirstSteps);
                }
            }
            return;
        }
        let stage = self.profile.stage;
        match stage {
            Stage::Swaddled => {
                self.quest.progress = (self.swaddled_for / SWADDLE_SECONDS).min(1.0);
                if self.swaddled_for >= SWADDLE_SECONDS {
                    self.enter(Stage::FirstSteps);
                }
            }
            Stage::FirstSteps => {
                let skill = self.profile.gait;
                self.quest.progress = (skill.level / WALK_THRESHOLD).min(1.0);
                if skill.level >= WALK_THRESHOLD {
                    self.enter(Stage::Foraging);
                }
            }
            Stage::Foraging => {
                // The quest is food, and the world's own food is the measure of
                // it. Three is not one: a fly that finds a single crumb has not
                // worked out how to feed herself.
                self.quest.progress = (ate as f32 / 3.0).min(1.0);
                if ate >= 3 {
                    self.enter(Stage::Lost);
                }
            }
            Stage::Lost => {
                let far = self.family.is_away(position);
                self.quest.progress = if far { 1.0 } else { 0.0 };
                if far {
                    self.enter(Stage::Homecoming);
                }
            }
            Stage::Homecoming => {
                let home = self.family.is_home(position);
                self.quest.progress = if home { 1.0 } else { 0.0 };
                if home {
                    self.enter(Stage::Adventurer);
                }
            }
            Stage::Adventurer => {
                self.quest.progress = 1.0;
            }
        }
    }

    /// Move to a chapter, and set up the quest and the task of the one after it.
    ///
    /// It deliberately does *not* put her to sleep. That was the first version,
    /// on the reasonable-sounding argument that a chapter change is a good moment
    /// to consolidate. It meant a newborn who opened her eyes was instantly
    /// asleep, so the chapter after that one was never ticked, so the life
    /// stopped two chapters in. Deciding when to sleep belongs to the fly's
    /// energy and to whoever is watching, not to the chapter machine.
    fn enter(&mut self, next: Stage) {
        let from = self.profile.stage;
        if next == from {
            return;
        }
        self.profile.stage = next;
        self.profile.stage_progress = 0.0;
        self.task = task_for(next);
        self.quest = quest_for(next);
        self.last_change = Some(format!("{} → {}", from.name(), next.name()));
        self.dirty = true;
    }

    /// Take the flag, so the runtime saves once and the flag does not stay set.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}

/// The task a chapter trains.
///
/// The order is the order the chapters happen in, and it is not the order the
/// tasks are listed in. A newborn has hands and eyes before she has a plan, so
/// she plays Tetris before she is lost in a maze, and the maze comes before the
/// chess because being lost comes before thinking ahead. Melody and code are
/// last because both need a memory of something that is not in front of her,
/// and that is exactly what the earlier chapters are for.
pub fn task_for(stage: Stage) -> Task {
    match stage {
        Stage::Swaddled | Stage::FirstSteps => Task::Tetris,
        Stage::Foraging | Stage::Lost => Task::Maze,
        Stage::Homecoming => Task::Drawing,
        Stage::Adventurer => Task::Melody,
    }
}

/// The quest of a chapter.
pub fn quest_for(stage: Stage) -> Quest {
    match stage {
        Stage::Swaddled => Quest::new("лежит в пелёнках", "открывает глаза"),
        Stage::FirstSteps => Quest::new("учится ходить", "делает первые шаги"),
        Stage::Foraging => Quest::new("ест", "выходит из дома за едой"),
        Stage::Lost => Quest::new("ушла далеко", "не знает дороги домой"),
        Stage::Homecoming => Quest::new("идёт домой", "находит дорогу"),
        Stage::Adventurer => Quest::new("сама идёт куда хочет", "ищет приключений"),
    }
}

/// Three decimals, without the trailing zeroes a formatted float would bring.
fn trim(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 0.005 {
        format!("{}", rounded.round() as i32)
    } else {
        format!("{rounded:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fly with everything switched off, so a test is about the life and not
    /// about whatever neurochemistry a random fly happens to start with.
    fn quiet() -> Fly {
        let mut fly = Fly::new();
        fly.set_learn_rate(1.0);
        fly
    }

    #[test]
    fn the_senses_fold_into_a_cue_the_brain_has() {
        // Every task state has to produce a cue the C core can index. A value
        // outside the twelve is clamped silently by the core, and a fly learning
        // against a clamped cue is learning against a cue that is not the one she
        // was given.
        for task in Task::ALL {
            for level in [0.0, 0.5, 1.0] {
                let mut episode = Episode::new(task, level, 1, 1);
                for _ in 0..40 {
                    let senses = episode.observe();
                    let cue = cue_of(&senses);
                    assert!(
                        (0..12).contains(&cue),
                        "{task:?} produced cue {cue}, which the core has no slot for"
                    );
                    if episode.over() {
                        break;
                    }
                    episode.act(0);
                }
            }
        }
    }

    #[test]
    fn a_bright_room_and_a_dark_one_are_different_cues() {
        // If these folded to the same cue, a fly could not tell day from night and
        // half the table would be wasted.
        //
        // The dark room has something else in it. That is not a convenience: a
        // room where light is at zero and all three other channels are at zero is
        // not a dark room, it is an absence of information, and the two have to
        // come out different or a fly standing in the dark would be told she was
        // in a bright room by the `1 - light` transform.
        let bright = Senses {
            light: 0.95,
            odor: 0.0,
            touch: 0.0,
            temperature: 0.0,
        };
        let dark_with_something_in_it = Senses {
            light: 0.02,
            odor: 0.0,
            touch: 0.5,
            temperature: 0.0,
        };
        assert_eq!(cue_of(&bright), tfly::cue::LIGHT);
        // A dark room she is touching something in is a touch, not a darkness:
        // darkness is a statement about how much light there is, and when
        // something else is what she can feel, that something is the news.
        assert_eq!(cue_of(&dark_with_something_in_it), tfly::cue::TOUCH);

        // Dim rather than dark. There is something to feel, but very little light,
        // so the cue is darkness rather than brightness.
        let dim = Senses {
            light: 0.15,
            odor: 0.0,
            touch: 0.12,
            temperature: 0.0,
        };
        assert_eq!(cue_of(&dim), tfly::cue::DARK);
    }

    #[test]
    fn a_quiet_room_is_a_resting_cue_and_not_a_wall_of_light() {
        // Everything at zero has to land somewhere. It used to land on LIGHT,
        // because a maximum on an empty vector is arbitrary, and a fly dozing in a
        // dark room appeared to be in a bright one.
        let still = Senses::default();
        assert_eq!(cue_of(&still), tfly::cue::VIBRATION);
    }

    #[test]
    fn every_motor_a_task_can_choose_is_one_the_fly_has() {
        for task in Task::ALL {
            let actions = Episode::new(task, 0.5, 1, 1).actions();
            for index in 0..actions {
                let m = motor(index);
                assert!(
                    (0..10).contains(&m),
                    "{task:?} chose motor {m}, out of range"
                );
            }
        }
    }

    #[test]
    fn a_newborn_lies_in_swaddling_and_not_anything_else() {
        let life = Life::new(Profile::newborn("a", "Клава"));
        assert_eq!(life.stage(), Stage::Swaddled);
        assert_eq!(life.quest.text, "лежит в пелёнках");
        assert!(!life.is_asleep(), "she is awake, merely unable to move");
        assert!(life.family.len() >= 2, "she was born into a household");
    }

    #[test]
    fn she_opens_her_eyes_after_being_laid_down() {
        let mut fly = quiet();
        let mut life = Life::new(Profile::newborn("a", "Клава"));
        let mut guard = 0;
        while life.stage() == Stage::Swaddled && guard < 20_000 {
            life.tick(&mut fly, 0.05, life.family.home, 0, 0.1);
            guard += 1;
        }
        assert_eq!(
            life.stage(),
            Stage::FirstSteps,
            "she stayed in swaddling for {guard} ticks"
        );
        assert!(
            (1.0..=20.0).contains(&(guard as f32 * 0.05)),
            "and it took {guard} ticks, which is not the eighteen seconds it should"
        );
    }

    #[test]
    fn every_chapter_opens_when_its_quest_is_done_and_not_before() {
        // Each gate in turn, with the world set up for it and nothing else, so
        // that a gate which opens too early is caught as surely as one that never
        // opens.
        let mut fly = quiet();

        // First steps: the gait, and nothing else.
        let mut life = Life::new(Profile::newborn("a", "Клава"));
        life.swaddled_for = SWADDLE_SECONDS;
        life.tick(&mut fly, 0.05, life.family.home, 0, 0.1);
        assert_eq!(life.stage(), Stage::FirstSteps);
        life.tick(&mut fly, 0.05, life.family.home, 0, 0.1);
        assert_eq!(
            life.stage(),
            Stage::FirstSteps,
            "she left before she could walk"
        );
        life.profile.gait = crate::profile::Skill {
            level: WALK_THRESHOLD,
            progress: 1,
            best: 1,
        };
        life.tick(&mut fly, 0.05, life.family.home, 0, 0.1);
        assert_eq!(life.stage(), Stage::Foraging);

        // Foraging: food, counted by the world.
        life.tick(&mut fly, 0.05, life.family.home, 2, 0.1);
        assert_eq!(life.stage(), Stage::Foraging, "two scraps is not a meal");
        life.tick(&mut fly, 0.05, life.family.home, 3, 0.1);
        assert_eq!(life.stage(), Stage::Lost, "three is");

        // Lost: distance from the house.
        life.tick(&mut fly, 0.05, [40.0, 0.0, 0.0], 3, 0.1);
        assert_eq!(life.stage(), Stage::Homecoming);
        life.tick(&mut fly, 0.05, life.family.home, 3, 0.1);
        assert_eq!(life.stage(), Stage::Adventurer, "she is home");

        life.tick(&mut fly, 0.05, [0.0, 0.0, 0.0], 3, 0.1);
        assert_eq!(
            life.stage(),
            Stage::Adventurer,
            "and she stays an adventurer"
        );
    }

    #[test]
    fn each_chapter_trains_the_task_that_chapter_is_about() {
        // The curriculum. A fly that learned her maze before her first steps would
        // be a different and much harder learner, and this is the decision that
        // makes the order what it is.
        assert_eq!(task_for(Stage::Swaddled), Task::Tetris);
        assert_eq!(task_for(Stage::FirstSteps), Task::Tetris);
        assert_eq!(task_for(Stage::Foraging), Task::Maze);
        assert_eq!(task_for(Stage::Lost), Task::Maze);
        assert_eq!(task_for(Stage::Homecoming), Task::Drawing);
        assert_eq!(task_for(Stage::Adventurer), Task::Melody);
        // The two hardest are never handed out by the chapter machine. She has to
        // reach them, and at the end there is no chapter left, so they are reached
        // by choice in the editor.
        assert!(
            !Stage::ALL
                .iter()
                .any(|s| task_for(*s) == Task::Chess || task_for(*s) == Task::Code)
        );
    }

    #[test]
    fn a_sleep_runs_the_task_and_ends_by_itself() {
        let mut fly = quiet();
        let mut life = Life::new(Profile::newborn("a", "Клава"));
        life.sleep(&mut fly, 3);
        assert!(life.is_asleep());
        assert_eq!(life.dream.as_ref().map(|d| d.task()), Some(Task::Tetris));
        let mut guard = 0;
        while life.is_asleep() && guard < 1000 {
            life.dream_step(&mut fly, 0.2);
            guard += 1;
        }
        assert!(!life.is_asleep(), "the sleep never ended");
        assert_eq!(life.history.len(), 1, "and it was not written down");
        assert!(life.history[0].turns > 0);
    }

    #[test]
    fn zzz_does_the_primitive_work_at_all() {
        // The whole module rests on one claim: that associating a cue with an
        // action after the fly has performed it moves a weight. If that is false,
        // every skill number in this project is decoration.
        for steps in [1usize, 2, 6, 30] {
            let mut fly = Fly::new();
            fly.set_learn_rate(1.0);
            let before = fly.weight(tfly::cue::LIGHT, tfly::action::FORWARD);
            fly.clear_actions();
            fly.act(tfly::action::FORWARD, 1.0);
            fly.steps(steps as i32, 1.0 / 60.0);
            fly.associate(tfly::cue::LIGHT, tfly::action::FORWARD, 1.0, 1.0);
            let after = fly.weight(tfly::cue::LIGHT, tfly::action::FORWARD);
            println!(
                "steps={steps}: {before:.8} -> {after:.8} (delta {:.8})",
                after - before
            );
        }
        // And with no performance at all, for contrast.
        let mut fly = Fly::new();
        fly.associate(tfly::cue::LIGHT, tfly::action::FORWARD, 1.0, 1.0);
        println!(
            "no act: {} -> {}",
            0.0,
            fly.weight(tfly::cue::LIGHT, tfly::action::FORWARD)
        );
    }

    #[test]
    fn a_sleep_changes_a_real_weight_in_the_fly() {
        // The claim the whole module exists for: a sleep is not a counter going up
        // in a file, it is a change in the association table the fly chooses with.
        // Checked by measuring the table and not the profile.
        let mut fly = quiet();
        let mut dream = Dream::new(Task::Tetris, 0.0, 5, 0);
        let before = table_mass(&fly);
        let mut turns = 0u32;
        let mut paid = 0u32;
        while !dream.done() && turns < 400 {
            if dream.step(&mut fly, 0.2) != 0.0 {
                paid += 1;
            }
            turns += 1;
        }
        let after = table_mass(&fly);
        println!("sleep: {turns} turns, {paid} of them paid, table {before:.8} -> {after:.8}");
        assert!(turns > 10, "the sleep was {turns} turns long");
        assert!(
            paid > 0,
            "not one turn out of {turns} paid anything, so there was nothing to learn from"
        );
        assert!(
            (after - before).abs() > 1e-6,
            "a whole sleep left the association table untouched: {before} then {after}"
        );
    }

    /// The sum of every weight in the table.
    ///
    /// A plain nested loop rather than an iterator chain: the chain needs a `move`
    /// closure for the loop variable, and that closure then also captures the fly
    /// by move, which is not a borrow and does not compile.
    fn table_mass(fly: &Fly) -> f32 {
        let mut total = 0.0;
        for cue in 0..12 {
            for action in 0..10 {
                total += fly.weight(cue, action).abs();
            }
        }
        total
    }

    #[test]
    fn a_sleep_moves_the_skill_and_a_bad_one_does_not_move_it_down() {
        let mut fly = quiet();
        let mut life = Life::new(Profile::newborn("a", "Клава"));
        for _ in 0..6 {
            life.sleep(&mut fly, 7);
            while life.is_asleep() {
                life.dream_step(&mut fly, 0.25);
            }
        }
        let tetris = life.profile.skill(Task::Tetris);
        assert!(tetris.level > 0.0, "six sleeps taught her nothing at all");
        assert!(tetris.level <= 1.0);
        assert!(life.profile.sleeps >= 6, "the sleeps were not counted");

        // And a worse run must not take it away, which is why `gain` is clamped
        // at zero rather than allowed to go negative.
        let mut dream = Dream::new(Task::Tetris, 0.0, 1, 0);
        dream.reward = -100.0;
        assert_eq!(
            dream.gain(),
            0.0,
            "a terrible sleep was worth negative skill"
        );
    }

    #[test]
    fn a_better_level_makes_a_harder_sleep() {
        // The curriculum has to be a curriculum, or six sleeps at the same
        // difficulty are six of the same thing.
        let easy = Dream::new(Task::Maze, 0.0, 1, 0);
        let hard = Dream::new(Task::Maze, 1.0, 1, 0);
        assert!(
            hard.episode.max_steps() > easy.episode.max_steps(),
            "a harder level did not give a longer sleep"
        );
    }

    #[test]
    fn she_only_keeps_what_the_level_says() {
        let mut fly = quiet();
        let mut life = Life::new(Profile::newborn("a", "Клава"));
        life.task = Task::Code;
        life.sleep(&mut fly, 1);
        assert_eq!(life.dream.as_ref().map(|d| d.task()), Some(Task::Code));
    }

    #[test]
    fn a_household_is_four_people_with_names_and_a_house() {
        let family = Family::new();
        assert_eq!(family.len(), 4);
        assert!(family.members.iter().all(|m| !m.name.is_empty()));
        assert!(family.members.iter().all(|m| !m.role.is_empty()));
        assert!(family.members.iter().all(|m| !m.doing.is_empty()));
        assert!(family.summary().contains("мать"));
        // And the house is in the village, which is where the world puts it.
        assert!(family.is_home(family.home));
        assert!(!family.is_away(family.home));
        assert!(family.is_away([40.0, 0.0, 0.0]));
    }

    #[test]
    fn a_quest_always_says_what_she_is_doing_and_what_next() {
        for stage in Stage::ALL {
            let quest = quest_for(stage);
            assert!(!quest.text.is_empty(), "{stage:?} has an empty quest");
            assert!(!quest.after.is_empty(), "{stage:?} has no next thing");
            assert!(!quest.done, "a fresh quest is not already done");
        }
    }

    #[test]
    fn a_newborns_profile_survives_the_life_being_ticked() {
        // Cheap, and it is the test that would catch a change writing a skill
        // index out of range: the profile has one slot per task and the life
        // indexes them by chapter.
        let mut fly = quiet();
        let mut life = Life::new(Profile::newborn("a", "Клава"));
        for _ in 0..200 {
            life.tick(&mut fly, 0.05, [20.0, 0.0, 0.0], 3, 0.1);
        }
        for task in Task::ALL {
            let _ = life.profile.skill(task);
        }
        assert_eq!(life.profile.skills.len(), Task::ALL.len());
    }
}
