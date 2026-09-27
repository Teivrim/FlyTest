//! Profiles: a saved fly.
//!
//! A profile is one fly's whole life so far, on disk. The site opens on a list
//! of them, you pick one or make a new one, and the world it describes is the
//! world you walk into.
//!
//! ## Why this is a file and not the runtime
//!
//! The project has a hard hundred-megabyte memory cap and a rule against heavy
//! per-frame persistence. Both are respected here by never writing during a
//! tick. A profile is written when a life changes in a way worth keeping: it is
//! created, or a sleep ends, or the session is closed. A fly walking around her
//! village touches nothing on disk, so the cost of having a save system at all
//! is one write per sleep rather than sixty per second.
//!
//! ## Why one file per profile
//!
//! A single `profiles.json` holding every fly would be rewritten in full every
//! time any one of them was saved, which makes saving one fly cost the size of
//! all of them. One file each means a save is bounded by one life, and a
//! half-written save damages one profile rather than every profile.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The six things a fly learns while she sleeps.
///
/// These are the tasks her sleeping mind is put into, in the order she reaches
/// them. They are a list rather than a set because the order is a property of
/// the life: a fly is a better maze solver before she is a better chess player
/// because she learned to walk before she learned to plan, and a profile that
/// recorded the order could show that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    /// Turning pieces to clear lines. The first skill: it needs only a hand and
    /// an eye, which a newborn has.
    Tetris,
    /// Seeing three moves ahead. It needs holding a board in mind that is not
    /// in front of her, which is why it comes after the maze.
    Chess,
    /// Finding the way out of a place. The first task that needs memory, and the
    /// one that most resembles being lost.
    Maze,
    /// Putting a mark where a thing should be. The first task with no right
    /// answer, only a closer one.
    Drawing,
    /// Choosing the next note. The first task where the reward is not visible at
    /// the moment of the choice.
    Melody,
    /// Writing something that has to satisfy rules she cannot see through.
    /// The hardest, and the last, because it is the only one where being wrong
    /// is not obvious until much later.
    Code,
}

impl Task {
    /// Every task, in the order they are learned.
    pub const ALL: [Task; 6] = [
        Task::Tetris,
        Task::Chess,
        Task::Maze,
        Task::Drawing,
        Task::Melody,
        Task::Code,
    ];

    /// The name the site shows.
    pub fn name(self) -> &'static str {
        match self {
            Task::Tetris => "Тетрис",
            Task::Chess => "Шахматы",
            Task::Maze => "Лабиринт",
            Task::Drawing => "Рисование",
            Task::Melody => "Мелодии",
            Task::Code => "Код на C++",
        }
    }

    /// The one-line description the site shows under the name.
    pub fn about(self) -> &'static str {
        match self {
            Task::Tetris => "поворот фигур и чистые линии",
            Task::Chess => "три хода вперёд",
            Task::Maze => "выход из места, где запуталась",
            Task::Drawing => "метка там, где вещь",
            Task::Melody => "следующая нота",
            Task::Code => "правила, которых не видно",
        }
    }

    /// The wire name, so the client and the snapshot agree.
    pub fn key(self) -> &'static str {
        match self {
            Task::Tetris => "tetris",
            Task::Chess => "chess",
            Task::Maze => "maze",
            Task::Drawing => "drawing",
            Task::Melody => "melody",
            Task::Code => "code",
        }
    }

    /// Parse a wire name back.
    pub fn from_key(key: &str) -> Option<Task> {
        Task::ALL.into_iter().find(|task| task.key() == key)
    }
}

/// Where a fly is in her life.
///
/// The order is the order she lives through, and it is also what the site shows
/// as her current chapter, so the enum order and the Russian names must stay
/// together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// A newborn, in swaddling, in a house in the village.
    Swaddled,
    /// On the floor, learning to put one leg in front of the other.
    FirstSteps,
    /// Feeding herself, and going out of the house to do it.
    Foraging,
    /// Away from home and not sure which way is back. The stage the world is
    /// built around: the tree on the hill is out here.
    Lost,
    /// Going back, which she can do because of the stage before it.
    Homecoming,
    /// Out on her own, going to the city, the forest, the ruins, on purpose.
    Adventurer,
}

impl Stage {
    /// Every stage, in order.
    pub const ALL: [Stage; 6] = [
        Stage::Swaddled,
        Stage::FirstSteps,
        Stage::Foraging,
        Stage::Lost,
        Stage::Homecoming,
        Stage::Adventurer,
    ];

    /// The name the site shows.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Swaddled => "Пелёнки",
            Stage::FirstSteps => "Первые шаги",
            Stage::Foraging => "Еда",
            Stage::Lost => "Потерялась",
            Stage::Homecoming => "Дорога домой",
            Stage::Adventurer => "Искательница",
        }
    }

    /// What she is doing in this stage, in one line, for the site.
    pub fn doing(self) -> &'static str {
        match self {
            Stage::Swaddled => "лежит в пелёнках в доме",
            Stage::FirstSteps => "делает первые шаги по полу",
            Stage::Foraging => "выходит из дома за едой",
            Stage::Lost => "ушла далеко и не знает дороги",
            Stage::Homecoming => "идёт обратно к дому",
            Stage::Adventurer => "сама идёт туда, куда хочет",
        }
    }

    /// The chapter number, one-based, for the site.
    pub fn chapter(self) -> u8 {
        self as u8 + 1
    }

    /// The wire name.
    pub fn key(self) -> &'static str {
        match self {
            Stage::Swaddled => "swaddled",
            Stage::FirstSteps => "first_steps",
            Stage::Foraging => "foraging",
            Stage::Lost => "lost",
            Stage::Homecoming => "homecoming",
            Stage::Adventurer => "adventurer",
        }
    }

    /// Parse a wire name back.
    pub fn from_key(key: &str) -> Option<Stage> {
        Stage::ALL.into_iter().find(|stage| stage.key() == key)
    }
}

/// How good she is at one task, and what she is working on.
///
/// A skill is a number from 0 to 1 and a position in the task. Keeping both is
/// the point: a fly can be at 0.9 in Tetris and still be losing rows, because
/// the policy is shared and improving one task moves the weights that the other
/// task reads. Recording the two separately is what makes that visible instead
/// of mysterious.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    /// How well she does. 0 is a newborn, 1 is as good as this model gets.
    pub level: f32,
    /// How far through the task she is.
    pub progress: u32,
    /// Best she has ever done, which is not the same as the last thing.
    pub best: u32,
}

impl Default for Skill {
    fn default() -> Self {
        Skill {
            level: 0.0,
            progress: 0,
            best: 0,
        }
    }
}

/// One fly's saved life.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// The wire name, unique, lowercase, safe in a filename.
    pub id: String,
    /// What she is called, in Russian, as the player typed it.
    pub name: String,
    /// Simulated seconds of life lived so far.
    pub age: f32,
    /// Which chapter she is in.
    pub stage: Stage,
    /// How much of the current chapter is done, 0..1.
    pub stage_progress: f32,
    /// One entry per task, indexed like [`Task::ALL`].
    pub skills: Vec<Skill>,
    /// Which task she is sleeping into, when she is asleep.
    pub sleeping_into: Option<Task>,
    /// How many times she has slept, which is how many real training sessions
    /// she has had.
    pub sleeps: u32,
    /// How far she can walk unaided. The first skill, and the one the village
    /// is about.
    pub gait: Skill,
    /// Which district she is in.
    pub district: String,
    /// Where in that district, in world units.
    pub position: [f32; 3],
    /// Whether she has a family to go back to. Everyone starts with one.
    pub has_family: bool,
    /// Whether she has found the way home.
    pub found_home: bool,
    /// Which generation of her family she is. There is only ever one, but a save
    /// format that cannot say so cannot later hold more.
    pub generation: u32,
    /// When the profile was last written, in sim seconds, for the site to sort
    /// and display.
    pub saved_at: f32,
}

impl Profile {
    /// A newborn, in swaddling, in a house in the village.
    pub fn newborn(id: &str, name: &str) -> Profile {
        Profile {
            id: id.to_owned(),
            name: name.to_owned(),
            age: 0.0,
            stage: Stage::Swaddled,
            stage_progress: 0.0,
            skills: vec![Skill::default(); Task::ALL.len()],
            sleeping_into: None,
            sleeps: 0,
            gait: Skill::default(),
            // The village, not the hill. She is born at home; the hill with the
            // tree is out in the world, and getting there is the fourth chapter.
            district: "village".to_owned(),
            position: [13.0, 0.0, 0.0],
            has_family: true,
            found_home: false,
            generation: 1,
            saved_at: 0.0,
        }
    }

    /// One skill, by task.
    pub fn skill(&self, task: Task) -> Skill {
        let index = Task::ALL.iter().position(|t| *t == task).unwrap_or(0);
        self.skills.get(index).copied().unwrap_or_default()
    }

    /// Move a skill, keeping it in range.
    pub fn train(&mut self, task: Task, reward: f32, reached: u32) {
        let index = match Task::ALL.iter().position(|t| *t == task) {
            Some(index) => index,
            None => return,
        };
        if self.skills.len() != Task::ALL.len() {
            // A profile written by an older build, or a hand-edited one. Grow it
            // rather than dropping the fly's history on the floor.
            self.skills.resize(Task::ALL.len(), Skill::default());
        }
        let skill = &mut self.skills[index];
        skill.level = (skill.level + reward).clamp(0.0, 1.0);
        skill.progress = reached;
        if reached > skill.best {
            skill.best = reached;
        }
    }
}

/// What the site shows in the list, without loading whole profiles.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub age: f32,
    pub stage: Stage,
    pub stage_name: &'static str,
    pub doing: &'static str,
    pub chapter: u8,
    pub sleeps: u32,
    /// The three skills she is best at, so a list of six flies is a list of
    /// six different flies and not six copies of the same row.
    pub best_at: Vec<&'static str>,
    pub district: String,
    pub found_home: bool,
}

impl ProfileSummary {
    /// The short form, for the chooser.
    pub fn of(profile: &Profile) -> ProfileSummary {
        let mut ranked: Vec<(usize, Task)> = (0..profile.skills.len())
            .zip(Task::ALL.iter().copied())
            .filter(|(index, _)| profile.skills.get(*index).map_or(0.0, |s| s.level) > 0.01)
            .collect();
        ranked.sort_by(|a, b| {
            profile.skills[b.0]
                .level
                .partial_cmp(&profile.skills[a.0].level)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let best_at = ranked.iter().take(3).map(|(_, task)| task.name()).collect();
        ProfileSummary {
            id: profile.id.clone(),
            name: profile.name.clone(),
            age: profile.age,
            stage: profile.stage,
            stage_name: profile.stage.name(),
            doing: profile.stage.doing(),
            chapter: profile.stage.chapter(),
            sleeps: profile.sleeps,
            best_at,
            district: profile.district.clone(),
            found_home: profile.found_home,
        }
    }
}

/// One Cyrillic letter as Latin, or `None` if it has no sensible spelling.
///
/// A small table rather than a transliteration crate: the ids only have to be
/// readable and unique, and a table of the Russian alphabet is the whole of it.
/// `None` covers the letters that carry no English sound (`ъ`, `ь`), the ones
/// that are modifiers rather than sounds, and everything outside Russian.
///
/// It returns a string rather than a `char` because most Russian consonants are
/// not one English letter: `ж` is `zh`, `щ` is `sch`, `я` is `ya`. A `char`
/// return cannot hold them, and truncating to the first letter would make
/// `Клава` and `Хамов` collide on `k` and `h`.
fn transliterate(c: char) -> Option<&'static str> {
    let mut lower = c.to_lowercase();
    let first = lower.next().unwrap_or(c);
    if lower.next().is_some() {
        // A character whose lowercase is more than one character is not one of
        // the letters in the table, and not something to guess at.
        return None;
    }
    Some(match first {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' | 'ё' | 'э' => "e",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "sch",
        'ъ' | 'ь' => return None,
        'ы' => "y",
        'ю' => "yu",
        'я' => "ya",
        // Not Russian. Latin letters and digits are handled by the caller,
        // which already has the character in hand, and everything else, such as
        // an emoji a player put in a name, is dropped rather than spelled,
        // because there is no honest spelling of it.
        _ => return None,
    })
}

/// The profiles on disk.
#[derive(Debug)]
pub struct ProfileStore {
    root: PathBuf,
}
impl ProfileStore {
    /// A store under `root`, created if it is not there.
    pub fn new(root: impl Into<PathBuf>) -> std::io::Result<ProfileStore> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(ProfileStore { root })
    }

    /// Where the profiles live.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file one profile is stored in.
    fn file(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }

    /// Every profile, newest life first. Unreadable files are skipped rather
    /// than fatal: one bad save should not hide the rest of a player's flies.
    pub fn list(&self) -> Vec<Profile> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(_) => return out,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(profile) = self.read(&path) {
                out.push(profile);
            }
        }
        out.sort_by(|a, b| b.age.total_cmp(&a.age));
        out
    }

    /// Read one profile by id.
    pub fn load(&self, id: &str) -> Option<Profile> {
        self.read(&self.file(id))
    }

    /// Read a profile from a path, tolerating a bad file.
    fn read(&self, path: &Path) -> Option<Profile> {
        let text = std::fs::read_to_string(path).ok()?;
        match serde_json::from_str::<Profile>(&text) {
            Ok(mut profile) => {
                // A profile written before a task existed is short a skill. Pad
                // it, so an old save opens instead of failing to parse.
                if profile.skills.len() < Task::ALL.len() {
                    profile.skills.resize(Task::ALL.len(), Skill::default());
                }
                Some(profile)
            }
            Err(_) => None,
        }
    }

    /// Write one profile.
    ///
    /// The write goes to a temporary file and is then renamed over the old one.
    /// Renaming is atomic on the same filesystem, so a save interrupted by a
    /// crash leaves the previous save intact rather than a half-written file
    /// that parses as nothing.
    pub fn save(&mut self, profile: &Profile) -> std::io::Result<()> {
        let target = self.file(&profile.id);
        let temp = self.root.join(format!("{}.tmp", profile.id));
        let text = serde_json::to_string_pretty(profile)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &target)
    }

    /// Remove one profile. Its file and any leftover temporary go together.
    pub fn remove(&self, id: &str) -> std::io::Result<()> {
        let target = self.file(id);
        if target.exists() {
            std::fs::remove_file(target)?;
        }
        let temp = self.root.join(format!("{id}.tmp"));
        if temp.exists() {
            let _ = std::fs::remove_file(temp);
        }
        Ok(())
    }

    /// Turn a name a player typed into a filename-safe id, and make it unique.
    ///
    /// The name itself is kept, in Russian, on the profile; this is only the
    /// filename, and it has to be safe on every filesystem the project might be
    /// copied to. Two rules earn their place here:
    ///
    /// - Cyrillic is transliterated rather than dropped, so `Клава` becomes
    ///   `klava`. The alternative, keeping the original letters, is a filename
    ///   that is fine on Windows and a portability question everywhere else;
    ///   dropping them instead would make every Russian name collide on `fly`.
    /// - Everything else non-alphanumeric becomes a dash, and runs of dashes
    ///   collapse, so `Клава!` and `Клава?` cannot become two different flies
    ///   with the same name.
    pub fn make_id(&self, name: &str) -> String {
        let mut spelled = String::new();
        for c in name.chars() {
            // A dash is written only where something was dropped, and never
            // after a letter. Adding one after every letter as well would mean
            // `му ха` came out as `m-u--x-a`, and the split-and-join below would
            // then turn that into `m-u-x-a` rather than the `mu-xa` a reader
            // expects.
            if c.is_ascii_alphanumeric() {
                spelled.push(c.to_ascii_lowercase());
            } else if let Some(latin) = transliterate(c) {
                spelled.push_str(latin);
            } else {
                spelled.push('-');
            }
        }
        let base: String = spelled
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = base.to_ascii_lowercase();
        let base = if base.is_empty() {
            "fly".to_owned()
        } else {
            base[..base.len().min(40)].to_owned()
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while self.file(&candidate).exists() {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store in a fresh directory under the target dir, so the tests never
    /// touch a real player's profiles.
    fn store(tag: &str) -> (ProfileStore, PathBuf) {
        let mut root = std::env::temp_dir();
        root.push(format!("flytest-profiles-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        (ProfileStore::new(root.clone()).expect("store"), root)
    }

    #[test]
    fn a_newborn_starts_swaddled_at_home() {
        let profile = Profile::newborn("a", "Клава");
        assert_eq!(profile.stage, Stage::Swaddled);
        assert_eq!(
            profile.district, "village",
            "she is born at home, not on the hill"
        );
        assert!(profile.has_family, "she is born into a family");
        assert!(!profile.found_home, "there is nothing to find yet");
        assert_eq!(profile.skills.len(), Task::ALL.len(), "one slot per task");
        assert!(
            profile.skills.iter().all(|s| s.level == 0.0),
            "a newborn knows nothing"
        );
    }

    #[test]
    fn every_stage_and_task_has_a_russian_name_and_a_wire_name() {
        // The site shows these. A missing name is a blank row in the chooser and
        // a chapter with no title, so it is checked rather than trusted.
        for stage in Stage::ALL {
            assert!(!stage.name().is_empty());
            assert!(!stage.doing().is_empty());
            assert_eq!(Stage::from_key(stage.key()), Some(stage));
            assert!(stage.name().chars().all(|c| c.is_alphabetic() || c == ' '));
        }
        for task in Task::ALL {
            assert!(!task.name().is_empty());
            assert!(!task.about().is_empty());
            assert_eq!(Task::from_key(task.key()), Some(task));
        }
        // Six of each, all distinct, because a duplicate name is a chooser with
        // two identical rows and no way to tell the flies apart.
        let mut stages: Vec<&str> = Stage::ALL.iter().map(|s| s.name()).collect();
        stages.sort_unstable();
        stages.dedup();
        assert_eq!(stages.len(), 6);
        let mut tasks: Vec<&str> = Task::ALL.iter().map(|t| t.name()).collect();
        tasks.sort_unstable();
        tasks.dedup();
        assert_eq!(tasks.len(), 6);
    }

    #[test]
    fn skills_go_up_and_stay_in_range() {
        let mut profile = Profile::newborn("a", "Клава");
        profile.train(Task::Tetris, 0.3, 5);
        assert!((profile.skill(Task::Tetris).level - 0.3).abs() < 1e-6);
        profile.train(Task::Tetris, 5.0, 9);
        assert_eq!(profile.skill(Task::Tetris).level, 1.0, "clamped at the top");
        assert_eq!(profile.skill(Task::Tetris).progress, 9);
        assert_eq!(profile.skill(Task::Tetris).best, 9);
    }

    #[test]
    fn a_bad_reward_cannot_make_a_skill_negative() {
        let mut profile = Profile::newborn("a", "Клава");
        profile.train(Task::Maze, -4.0, 0);
        assert_eq!(profile.skill(Task::Maze).level, 0.0);
    }

    #[test]
    fn the_best_skill_is_remembered_even_after_a_bad_run() {
        let mut profile = Profile::newborn("a", "Клава");
        profile.train(Task::Chess, 0.8, 12);
        profile.train(Task::Chess, -0.5, 3);
        let chess = profile.skill(Task::Chess);
        assert_eq!(chess.best, 12, "the best run is not the last run");
        assert_eq!(
            chess.progress, 3,
            "but the last run is what she is doing now"
        );
    }

    #[test]
    fn a_profile_survives_a_round_trip_through_the_disk() {
        let (mut store, root) = store("roundtrip");
        let mut profile = Profile::newborn("klava", "Клава");
        profile.train(Task::Melody, 0.4, 3);
        profile.stage = Stage::Adventurer;
        profile.sleeps = 7;
        store.save(&profile).expect("save");
        let back = store.load("klava").expect("load");
        assert_eq!(back, profile, "what comes back is what went in");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_profile_from_an_older_build_is_padded_rather_than_rejected() {
        let (store, root) = store("older");
        // Three skills, because it was written before there were six.
        let short = r#"{
          "id": "old", "name": "Старая", "age": 100.0, "stage": "foraging",
          "stage_progress": 0.5,
          "skills": [
            {"level": 0.5, "progress": 2, "best": 3},
            {"level": 0.2, "progress": 1, "best": 1},
            {"level": 0.1, "progress": 0, "best": 0}
          ],
          "sleeping_into": null, "sleeps": 2,
          "gait": {"level": 0.4, "progress": 9, "best": 11},
          "district": "village", "position": [13.0, 0.0, 0.0],
          "has_family": true, "found_home": false, "generation": 1, "saved_at": 0.0
        }"#;
        let path = root.join("old.json");
        std::fs::write(&path, short).expect("write");
        let loaded = store.load("old").expect("an old save must still open");
        assert_eq!(loaded.skills.len(), 6, "padded to the current task count");
        assert_eq!(
            loaded.skill(Task::Tetris).level,
            0.5,
            "the old skills kept their places"
        );
        assert_eq!(
            loaded.skill(Task::Code).level,
            0.0,
            "the new ones start at nothing"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_corrupt_file_is_skipped_and_the_others_still_load() {
        let (mut store, root) = store("corrupt");
        let mut good = Profile::newborn("good", "Хорошая");
        store.save(&good).expect("save");
        std::fs::write(root.join("bad.json"), "{ not json at all").expect("write");
        let list = store.list();
        assert_eq!(list.len(), 1, "one bad file does not hide the rest");
        assert_eq!(list[0].id, "good");
        good.id = String::new();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_newest_life_is_listed_first() {
        let (mut store, root) = store("order");
        let mut young = Profile::newborn("young", "Молодая");
        young.age = 10.0;
        let mut old = Profile::newborn("old", "Старая");
        old.age = 900.0;
        store.save(&young).expect("save");
        store.save(&old).expect("save");
        let list = store.list();
        assert_eq!(list[0].id, "old", "the longer life is first");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn two_flies_with_the_same_name_get_different_ids() {
        let (mut store, root) = store("ids");
        let mut first = Profile::newborn("klava", "Клава");
        first.id = store.make_id("Клава");
        store.save(&first).expect("save");
        let second = store.make_id("Клава");
        assert_ne!(first.id, second, "the second Клава is a different fly");
        assert_eq!(first.id, "klava", "readable, and safe in a filename");
        assert!(second.is_ascii(), "no cyrillic in a filename");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_name_with_nothing_usable_in_it_still_produces_an_id() {
        let (store, root) = store("weird");
        assert_eq!(store.make_id("!!! ???"), "fly", "not an empty filename");
        assert_eq!(store.make_id(""), "fly");
        assert_eq!(
            store.make_id("Муха 01"),
            "mukha-01",
            "transliterated, not dropped"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_summary_shows_the_three_skills_she_is_best_at() {
        let mut profile = Profile::newborn("a", "Клава");
        profile.train(Task::Chess, 0.9, 1);
        profile.train(Task::Tetris, 0.5, 1);
        profile.train(Task::Maze, 0.7, 1);
        let summary = ProfileSummary::of(&profile);
        assert_eq!(
            summary.best_at,
            vec!["Шахматы", "Лабиринт", "Тетрис"],
            "in order of level"
        );
        assert_eq!(summary.chapter, 1);
    }

    #[test]
    fn a_newborn_summary_lists_no_skills() {
        // An empty row of "what she is good at" is worse than saying nothing:
        // the chooser would show six flies that all look identical.
        let summary = ProfileSummary::of(&Profile::newborn("a", "Клава"));
        assert!(summary.best_at.is_empty());
    }
}
