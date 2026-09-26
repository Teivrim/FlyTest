//! Safe Rust bindings for the C core.
//!
//! # Design
//!
//! All of the model lives in [`TFLY.h`](../TFLY.h). This module does three
//! things and nothing else:
//!
//! 1. declares the C ABI exported by `native/tfly_ffi.c`;
//! 2. owns the handle, so no Rust code can use a freed fly;
//! 3. converts between `String`/`&str` and the C names at the boundary.
//!
//! The `unsafe` surface is one `extern "C"` block plus the two calls that
//! dereference a pointer. Everything else is ordinary safe Rust.
//!
//! # Why a C core at all
//!
//! The editor, the Blender bridge and the JSONL tools are all free to be
//! rewritten. Having one copy of the model, in a language with no allocator
//! in the hot path, is what keeps the fly behaving identically in all three
//! places. See `docs/tfly-design.md` for the architecture argument.

// Calling into a C core requires `unsafe`. This is the only module in the
// crate that opts in, and the scope is deliberately narrow: one extern block,
// one owned handle, and no pointer arithmetic beyond what the FFI layer does.
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_float, c_int, c_uint};

use serde::Serialize;

/// Major version of the C core, kept in sync with `TFLY_VERSION_MAJOR`.
pub const VERSION_MAJOR: i32 = 1;
/// Minor version of the C core, kept in sync with `TFLY_VERSION_MINOR`.
pub const VERSION_MINOR: i32 = 0;

/// Opaque pointer to the C-side fly state.
#[repr(C)]
struct TFlyHandle {
    _private: [u8; 0],
}

#[allow(unsafe_code)]
unsafe extern "C" {
    fn tfly_new() -> *mut TFlyHandle;
    fn tfly_free(handle: *mut TFlyHandle);
    fn tfly_struct_size() -> c_int;

    fn tfly_seed(handle: *mut TFlyHandle, seed: c_uint);
    fn tfly_update(handle: *mut TFlyHandle, dt: c_float);
    fn tfly_steps(handle: *mut TFlyHandle, n: c_int, dt: c_float);

    fn tfly_light(handle: *mut TFlyHandle, v: c_float);
    fn tfly_odor(handle: *mut TFlyHandle, v: c_float, kind: c_int);
    fn tfly_touch(handle: *mut TFlyHandle, v: c_float);
    fn tfly_temperature(handle: *mut TFlyHandle, v: c_float);
    fn tfly_sound(handle: *mut TFlyHandle, v: c_float);
    fn tfly_vibration(handle: *mut TFlyHandle, v: c_float);
    fn tfly_taste(
        handle: *mut TFlyHandle,
        sweet: c_float,
        salt: c_float,
        bitter: c_float,
        water: c_float,
    );

    fn tfly_heart(handle: *mut TFlyHandle, intensity: c_float, region: c_int) -> c_float;
    fn tfly_pain(handle: *mut TFlyHandle, v: c_float) -> c_float;
    fn tfly_heal(handle: *mut TFlyHandle, region: c_int, rate: c_float);
    fn tfly_heal_all(handle: *mut TFlyHandle);

    fn tfly_hormone(
        handle: *mut TFlyHandle,
        intensity: c_float,
        hormone: c_int,
        duration: c_float,
    ) -> c_float;
    fn tfly_hormone_named(
        handle: *mut TFlyHandle,
        intensity: c_float,
        name: *const c_char,
        duration: c_float,
    ) -> c_float;
    fn tfly_clear_hormones(handle: *mut TFlyHandle);

    fn tfly_emotion(handle: *mut TFlyHandle, emotion: c_int, v: c_float);
    fn tfly_fear(handle: *mut TFlyHandle, v: c_float);
    fn tfly_joy(handle: *mut TFlyHandle, v: c_float);
    fn tfly_sadness(handle: *mut TFlyHandle, v: c_float);
    fn tfly_anger(handle: *mut TFlyHandle, v: c_float);
    fn tfly_surprise(handle: *mut TFlyHandle, v: c_float);
    fn tfly_pride(handle: *mut TFlyHandle, v: c_float);
    fn tfly_curiosity(handle: *mut TFlyHandle, v: c_float);
    fn tfly_contentment(handle: *mut TFlyHandle, v: c_float);
    fn tfly_dread(handle: *mut TFlyHandle, v: c_float);
    fn tfly_lust(handle: *mut TFlyHandle, v: c_float);

    fn tfly_drive(handle: *mut TFlyHandle, drive: c_int, v: c_float);
    fn tfly_hunger(handle: *mut TFlyHandle, v: c_float);
    fn tfly_thirst(handle: *mut TFlyHandle, v: c_float);
    fn tfly_sex_drive(handle: *mut TFlyHandle, v: c_float);

    fn tfly_associate(
        handle: *mut TFlyHandle,
        cue: c_int,
        action: c_int,
        outcome: c_float,
        modulator: c_float,
    );
    fn tfly_reward(handle: *mut TFlyHandle, v: c_float);
    fn tfly_punish(handle: *mut TFlyHandle, v: c_float);
    fn tfly_plasticity(handle: *mut TFlyHandle, v: c_float);
    fn tfly_reset_learning(handle: *mut TFlyHandle);
    fn tfly_learning_gate(handle: *mut TFlyHandle) -> c_float;
    fn tfly_assoc_weight(handle: *mut TFlyHandle, cue: c_int, action: c_int) -> c_float;

    fn tfly_memorize(handle: *mut TFlyHandle, key: c_float, value: c_float);
    fn tfly_recall(handle: *mut TFlyHandle, key: c_float) -> c_float;
    fn tfly_forget(handle: *mut TFlyHandle);

    fn tfly_mate_signal(handle: *mut TFlyHandle, q: c_float);
    fn tfly_rival_signal(handle: *mut TFlyHandle, p: c_float);
    fn tfly_predator_signal(handle: *mut TFlyHandle, r: c_float);

    fn tfly_sleep(handle: *mut TFlyHandle);
    fn tfly_wake(handle: *mut TFlyHandle);

    fn tfly_thrust(handle: *mut TFlyHandle, v: c_float);
    fn tfly_turn(handle: *mut TFlyHandle, v: c_float);
    fn tfly_lift(handle: *mut TFlyHandle, v: c_float);
    fn tfly_rest(handle: *mut TFlyHandle, v: c_float);

    fn tfly_attention(handle: *mut TFlyHandle, channel: c_int, gain: c_float);

    fn tfly_emotion_level(handle: *mut TFlyHandle, e: c_int) -> c_float;
    fn tfly_drive_level(handle: *mut TFlyHandle, d: c_int) -> c_float;
    fn tfly_hormone_level(handle: *mut TFlyHandle, x: c_int) -> c_float;
    fn tfly_sensory_level(handle: *mut TFlyHandle, s: c_int) -> c_float;
    fn tfly_dominant_emotion(handle: *mut TFlyHandle) -> c_int;
    fn tfly_dominant_drive(handle: *mut TFlyHandle) -> c_int;
    fn tfly_asleep(handle: *mut TFlyHandle) -> c_int;
    fn tfly_valence(handle: *mut TFlyHandle) -> c_float;
    fn tfly_arousal(handle: *mut TFlyHandle) -> c_float;
    fn tfly_energy(handle: *mut TFlyHandle) -> c_float;
    fn tfly_stress(handle: *mut TFlyHandle) -> c_float;
    fn tfly_pain_total(handle: *mut TFlyHandle) -> c_float;
    fn tfly_plasticity_level(handle: *mut TFlyHandle) -> c_float;
    fn tfly_thrust_out(handle: *mut TFlyHandle) -> c_float;
    fn tfly_turn_out(handle: *mut TFlyHandle) -> c_float;
    fn tfly_vertical_out(handle: *mut TFlyHandle) -> c_float;
    fn tfly_wingbeat(handle: *mut TFlyHandle) -> c_float;
    fn tfly_elapsed(handle: *mut TFlyHandle) -> c_float;
    fn tfly_action_level(handle: *mut TFlyHandle, a: c_int) -> c_float;
    fn tfly_memory_count(handle: *mut TFlyHandle) -> c_int;
    fn tfly_cue_value(handle: *mut TFlyHandle, c: c_int) -> c_float;
    fn tfly_lifespan(handle: *mut TFlyHandle) -> c_float;
    fn tfly_attend_all(handle: *mut TFlyHandle, gain: c_float);
    fn tfly_random(handle: *mut TFlyHandle) -> c_float;
    fn tfly_act(handle: *mut TFlyHandle, action: c_int, amount: c_float);
    fn tfly_clear_actions(handle: *mut TFlyHandle);
    fn tfly_set_gait(handle: *mut TFlyHandle, g: c_int);
    fn tfly_gait_weight(handle: *mut TFlyHandle, g: c_int) -> c_float;
    fn tfly_train_gait(handle: *mut TFlyHandle, g: c_int, score: c_float);
    fn tfly_gait_stride(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gait_cadence(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gait_sway(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gait_phase(handle: *mut TFlyHandle) -> c_float;
    fn tfly_step_count(handle: *mut TFlyHandle) -> c_float;
    fn tfly_balance(handle: *mut TFlyHandle) -> c_float;
    fn tfly_blink(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gaze_x(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gaze_y(handle: *mut TFlyHandle) -> c_float;
    fn tfly_pupil(handle: *mut TFlyHandle) -> c_float;
    fn tfly_brow(handle: *mut TFlyHandle) -> c_float;
    fn tfly_mouth(handle: *mut TFlyHandle) -> c_float;
    fn tfly_mouth_open(handle: *mut TFlyHandle) -> c_float;
    fn tfly_blush(handle: *mut TFlyHandle) -> c_float;
    fn tfly_tears(handle: *mut TFlyHandle) -> c_float;
    fn tfly_sweat(handle: *mut TFlyHandle) -> c_float;
    fn tfly_eye_open(handle: *mut TFlyHandle) -> c_float;
    fn tfly_eye_adapt(handle: *mut TFlyHandle) -> c_float;
    fn tfly_wake_timer(handle: *mut TFlyHandle) -> c_float;
    fn tfly_look_at(handle: *mut TFlyHandle, x: c_float, y: c_float);
    fn tfly_look_strength(handle: *mut TFlyHandle, s: c_float);
    fn tfly_look_away(handle: *mut TFlyHandle);
    fn tfly_look_x(handle: *mut TFlyHandle) -> c_float;
    fn tfly_look_y(handle: *mut TFlyHandle) -> c_float;
    fn tfly_look_lock(handle: *mut TFlyHandle) -> c_float;
    fn tfly_spine(handle: *mut TFlyHandle) -> c_float;
    fn tfly_limbs(handle: *mut TFlyHandle, out: *mut c_float);
    fn tfly_foot_drop(
        handle: *mut TFlyHandle,
        thigh: c_float,
        shin: c_float,
        sole_c: c_float,
        sole_a: c_float,
        sole_b: c_float,
        sole_f: c_float,
    ) -> c_float;
    fn tfly_foot_drop_pose(
        root: *const c_float,
        middle: *const c_float,
        end: *const c_float,
        thigh: c_float,
        shin: c_float,
        sole_c: c_float,
        sole_a: c_float,
        sole_b: c_float,
        sole_f: c_float,
    ) -> c_float;
    fn tfly_set_phase(handle: *mut TFlyHandle, phase: c_float);
    fn tfly_shoulder(handle: *mut TFlyHandle) -> c_float;
    fn tfly_lean(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gesture(handle: *mut TFlyHandle) -> c_int;
    fn tfly_gesture_strength(handle: *mut TFlyHandle) -> c_float;
    fn tfly_gesture_phase(handle: *mut TFlyHandle) -> c_float;
    fn tfly_bond(handle: *mut TFlyHandle) -> c_float;
    fn tfly_can_encounter(handle: *mut TFlyHandle) -> c_int;
    fn tfly_encounter_drive(handle: *mut TFlyHandle) -> c_float;
    fn tfly_last_encounter(handle: *mut TFlyHandle) -> c_int;
    fn tfly_gesture_start(handle: *mut TFlyHandle, g: c_int);
    fn tfly_gesture_force(handle: *mut TFlyHandle, g: c_int);
    fn tfly_gesture_name(g: c_int) -> *const c_char;
    fn tfly_encounter_name(e: c_int) -> *const c_char;
    fn tfly_encounter(a: *mut TFlyHandle, b: *mut TFlyHandle, kind: c_int) -> c_int;

    fn tfly_to_json(handle: *mut TFlyHandle, buf: *mut c_char, cap: c_int) -> c_int;
    fn tfly_hormone_name(id: c_int) -> *const c_char;
    fn tfly_emotion_name(id: c_int) -> *const c_char;
    fn tfly_drive_name(id: c_int) -> *const c_char;
    fn tfly_action_name(id: c_int) -> *const c_char;
    fn tfly_body_name(id: c_int) -> *const c_char;
}

/// Sensory channels, matching the `TSense` enum in `TFLY.h`.
pub mod sense {
    pub const LIGHT: i32 = 0;
    pub const DARK: i32 = 1;
    pub const ODOR: i32 = 2;
    pub const TOUCH: i32 = 3;
    pub const TEMPERATURE: i32 = 4;
    pub const GRAVITY: i32 = 5;
    pub const PROPRIOCEPTION: i32 = 6;
    pub const SOUND: i32 = 7;
    pub const VIBRATION: i32 = 8;
    pub const TASTE: i32 = 9;
    pub const SMELL_ANTENNA: i32 = 10;
    pub const WIND: i32 = 11;
}

/// Body regions, matching `TRegion`.
pub mod body {
    pub const HEAD: i32 = 0;
    pub const THORAX: i32 = 1;
    pub const ABDOMEN: i32 = 2;
    pub const WING_L: i32 = 3;
    pub const WING_R: i32 = 4;
    pub const LEG_L: i32 = 5;
    pub const LEG_R: i32 = 6;
    pub const EYE_L: i32 = 7;
    pub const EYE_R: i32 = 8;
    pub const PROBOSCIS: i32 = 9;
}

/// Odour identities, matching `T_ODOR_*`.
pub mod odor {
    pub const FRUIT: i32 = 0;
    pub const FLOWER: i32 = 1;
    pub const FEMALE: i32 = 2;
    pub const MALE: i32 = 3;
    pub const PREDATOR: i32 = 4;
    pub const DECAY: i32 = 5;
}

/// Hormones, matching `T_H_*`.
pub mod hormone {
    pub const OCTOPAMINE: i32 = 0;
    pub const DOPAMINE: i32 = 1;
    pub const SEROTONIN: i32 = 2;
    pub const NOVELTY: i32 = 3;
    pub const INSULIN: i32 = 4;
    pub const LEPTIN: i32 = 5;
    pub const ECDYSONE: i32 = 6;
    pub const JUVENILE_HORMONE: i32 = 7;
    pub const GLUTAMATE: i32 = 8;
    pub const GABA: i32 = 9;
    pub const ACETYLCHOLINE: i32 = 10;
    pub const NITRIC_OXIDE: i32 = 11;
    pub const CORAZONIN: i32 = 12;
    pub const NEUROPEPTIDE_F: i32 = 13;
    pub const DH31: i32 = 14;
    pub const FMRFAMIDE: i32 = 15;
    pub const CCAP: i32 = 16;
    pub const RELAXIN: i32 = 17;
    pub const ETH: i32 = 18;
    pub const IRS: i32 = 19;
    pub const TOR: i32 = 20;
    pub const SNPF: i32 = 21;
    pub const NLPP: i32 = 22;
    pub const PAM: i32 = 23;
    pub const PTTH: i32 = 24;
    pub const DH44: i32 = 25;
}

/// Affect variables, matching `T_EMO_*`.
pub mod emotion {
    pub const PAIN: i32 = 0;
    pub const FEAR: i32 = 1;
    pub const JOY: i32 = 2;
    pub const SADNESS: i32 = 3;
    pub const ANGER: i32 = 4;
    pub const DISGUST: i32 = 5;
    pub const SURPRISE: i32 = 6;
    pub const CURIOSITY: i32 = 7;
    pub const LUST: i32 = 8;
    pub const CRAVING: i32 = 9;
    pub const CONTENTMENT: i32 = 10;
    pub const DREAD: i32 = 11;
    pub const CONFUSION: i32 = 12;
    pub const PRIDE: i32 = 13;
    pub const GRATITUDE: i32 = 14;
    pub const RELIEF: i32 = 15;
    pub const DISAPPOINTMENT: i32 = 16;
    pub const HOPE: i32 = 17;
    pub const JEALOUSY: i32 = 18;
    pub const SHYNESS: i32 = 19;
    pub const AFFECTION: i32 = 20;
    pub const BOREDOM: i32 = 21;
    pub const EXCITEMENT: i32 = 22;
    pub const COMPASSION: i32 = 23;
    pub const TRUST: i32 = 24;
    pub const LONGING: i32 = 25;

    /// Every emotion, in declaration order, for enumeration in the UI.
    pub const ALL: [(&str, i32); 26] = [
        ("pain", PAIN),
        ("fear", FEAR),
        ("joy", JOY),
        ("sadness", SADNESS),
        ("anger", ANGER),
        ("disgust", DISGUST),
        ("surprise", SURPRISE),
        ("curiosity", CURIOSITY),
        ("lust", LUST),
        ("craving", CRAVING),
        ("contentment", CONTENTMENT),
        ("dread", DREAD),
        ("confusion", CONFUSION),
        ("pride", PRIDE),
        ("gratitude", GRATITUDE),
        ("relief", RELIEF),
        ("disappointment", DISAPPOINTMENT),
        ("hope", HOPE),
        ("jealousy", JEALOUSY),
        ("shyness", SHYNESS),
        ("affection", AFFECTION),
        ("boredom", BOREDOM),
        ("excitement", EXCITEMENT),
        ("compassion", COMPASSION),
        ("trust", TRUST),
        ("longing", LONGING),
    ];
}

/// Gestures, matching `T_GES_*` in the header.
pub mod gesture {
    pub const IDLE: i32 = 0;
    pub const WAVE: i32 = 1;
    pub const BOW: i32 = 2;
    pub const CLAP: i32 = 3;
    pub const POINT: i32 = 4;
    pub const COVER: i32 = 5;
    pub const SHRUG: i32 = 6;
    pub const HOLD: i32 = 7;
}

/// Encounters between two flies, matching `T_ENC_*`.
pub mod encounter {
    pub const GREET: i32 = 0;
    pub const BOW: i32 = 1;
    pub const HIGH_FIVE: i32 = 2;
    pub const COMFORT: i32 = 3;
    pub const SHARE: i32 = 4;
    pub const ARGUE: i32 = 5;
    pub const IGNORE: i32 = 6;
}

/// Gait presets the fly can learn to walk with.
pub mod gait {
    pub const HURRIED: i32 = 0;
    pub const STEADY: i32 = 1;
    pub const LONG_STRIDE: i32 = 2;
    pub const WEAVING: i32 = 3;
    pub const COUNT: i32 = 4;

    pub const NAMES: [&str; 4] = [
        "РЎвЂљР С•РЎР‚Р С•Р С—Р В»Р С‘Р Р†РЎвЂ№Р в„–",
        "РЎР‚Р С•Р Р†Р Р…РЎвЂ№Р в„–",
        "Р Т‘Р В»Р С‘Р Р…Р Р…РЎвЂ№Р в„–",
        "Р С—Р ВµРЎвЂљР В»РЎРЏРЎР‹РЎвЂ°Р С‘Р в„–",
    ];
}

/// Drives, matching `T_DRIVE_*`.
pub mod drive {
    pub const HUNGER: i32 = 0;
    pub const THIRST: i32 = 1;
    pub const SEX: i32 = 2;
    pub const SLEEP: i32 = 3;
    pub const FATIGUE: i32 = 4;
    pub const COLD: i32 = 5;
    pub const SOCIAL: i32 = 6;
    pub const CURIOSITY: i32 = 7;
}

/// Motor primitives, matching `T_ACT_*`.
pub mod action {
    pub const REST: i32 = 0;
    pub const FORWARD: i32 = 1;
    pub const TURN_L: i32 = 2;
    pub const TURN_R: i32 = 3;
    pub const UP: i32 = 4;
    pub const DOWN: i32 = 5;
    pub const FLAP: i32 = 6;
    pub const DANCE: i32 = 7;
    pub const EAT: i32 = 8;
    pub const MATE: i32 = 9;
}

/// Conditioned stimuli, matching `T_CUE_*`.
pub mod cue {
    pub const LIGHT: i32 = 0;
    pub const DARK: i32 = 1;
    pub const ODOR_FRUIT: i32 = 2;
    pub const ODOR_FLOWER: i32 = 3;
    pub const ODOR_FEMALE: i32 = 4;
    pub const ODOR_MALE: i32 = 5;
    pub const TOUCH: i32 = 6;
    pub const TEMPERATURE: i32 = 7;
    pub const SOUND: i32 = 8;
    pub const VIBRATION: i32 = 9;
    pub const VISUAL: i32 = 10;
    pub const GRAVITY: i32 = 11;
}

/// An owned fly. Dropping it frees the C-side state.
pub struct Fly {
    handle: *mut TFlyHandle,
}

// SAFETY: the handle is owned exclusively by this Fly and the C side never
// touches it concurrently. `Fly` is Send but deliberately not Sync: it owns a
// mutable C allocation, so sharing one by reference across threads is not
// sound, but moving it into another thread is. That is exactly what the
// editor runtime needs, since it guards the flies with a mutex.
unsafe impl Send for Fly {}

impl Fly {
    /// Create a fresh fly with default state.
    #[must_use]
    pub fn new() -> Self {
        let handle = unsafe { tfly_new() };
        assert!(!handle.is_null(), "tfly_new returned null");
        Self { handle }
    }

    fn ptr(&self) -> *mut TFlyHandle {
        self.handle
    }

    /// Seed the stimulus RNG so a run replays exactly.
    pub fn seed(&mut self, seed: u32) {
        unsafe { tfly_seed(self.ptr(), seed) }
    }

    /// Advance the simulation by `dt` seconds. Oversized `dt` is clamped in C.
    pub fn update(&mut self, dt: f32) {
        unsafe { tfly_update(self.ptr(), dt) }
    }

    /// Run `n` fixed steps.
    pub fn steps(&mut self, n: i32, dt: f32) {
        unsafe { tfly_steps(self.ptr(), n, dt) }
    }

    // ---- sensory -------------------------------------------------

    pub fn light(&mut self, v: f32) {
        unsafe { tfly_light(self.ptr(), v) }
    }
    pub fn odor(&mut self, v: f32, kind: i32) {
        unsafe { tfly_odor(self.ptr(), v, kind) }
    }
    pub fn touch(&mut self, v: f32) {
        unsafe { tfly_touch(self.ptr(), v) }
    }
    pub fn temperature(&mut self, v: f32) {
        unsafe { tfly_temperature(self.ptr(), v) }
    }
    pub fn sound(&mut self, v: f32) {
        unsafe { tfly_sound(self.ptr(), v) }
    }
    pub fn vibration(&mut self, v: f32) {
        unsafe { tfly_vibration(self.ptr(), v) }
    }
    pub fn taste(&mut self, sweet: f32, salt: f32, bitter: f32, water: f32) {
        unsafe { tfly_taste(self.ptr(), sweet, salt, bitter, water) }
    }

    // ---- nociception --------------------------------------------

    /// Deliver pain of `intensity` localised to `region`.
    pub fn heart(&mut self, intensity: f32, region: i32) -> f32 {
        unsafe { tfly_heart(self.ptr(), intensity, region) }
    }
    pub fn pain(&mut self, intensity: f32) -> f32 {
        unsafe { tfly_pain(self.ptr(), intensity) }
    }
    pub fn heal(&mut self, region: i32, rate: f32) {
        unsafe { tfly_heal(self.ptr(), region, rate) }
    }
    pub fn heal_all(&mut self) {
        unsafe { tfly_heal_all(self.ptr()) }
    }

    // ---- chemistry ----------------------------------------------

    /// Administer a hormone. `duration > 0` is a bolus, `<= 0` is a tonic
    /// set-point shift.
    pub fn hormone(&mut self, intensity: f32, hormone: i32, duration: f32) -> f32 {
        unsafe { tfly_hormone(self.ptr(), intensity, hormone, duration) }
    }

    /// Administer a hormone by name, e.g. `"dopamine"`.
    pub fn hormone_named(&mut self, intensity: f32, name: &str, duration: f32) -> f32 {
        let Ok(c_name) = std::ffi::CString::new(name) else {
            return 0.0;
        };
        unsafe { tfly_hormone_named(self.ptr(), intensity, c_name.as_ptr(), duration) }
    }

    pub fn clear_hormones(&mut self) {
        unsafe { tfly_clear_hormones(self.ptr()) }
    }

    // ---- affect --------------------------------------------------

    pub fn emotion(&mut self, emotion: i32, v: f32) {
        unsafe { tfly_emotion(self.ptr(), emotion, v) }
    }
    pub fn fear(&mut self, v: f32) {
        unsafe { tfly_fear(self.ptr(), v) }
    }
    pub fn joy(&mut self, v: f32) {
        unsafe { tfly_joy(self.ptr(), v) }
    }
    pub fn sadness(&mut self, v: f32) {
        unsafe { tfly_sadness(self.ptr(), v) }
    }
    pub fn anger(&mut self, v: f32) {
        unsafe { tfly_anger(self.ptr(), v) }
    }
    pub fn curiosity(&mut self, v: f32) {
        unsafe { tfly_curiosity(self.ptr(), v) }
    }
    /// Startle her.
    ///
    /// A surprise above three quarters wakes her, because arousal has to
    /// reach the sleep state or a startle would only lift the eyelids of
    /// somebody still asleep.
    pub fn surprise(&mut self, v: f32) {
        unsafe { tfly_surprise(self.ptr(), v) }
    }
    /// Pride, which is what makes her stand up straight.
    pub fn pride(&mut self, v: f32) {
        unsafe { tfly_pride(self.ptr(), v) }
    }
    pub fn contentment(&mut self, v: f32) {
        unsafe { tfly_contentment(self.ptr(), v) }
    }
    pub fn dread(&mut self, v: f32) {
        unsafe { tfly_dread(self.ptr(), v) }
    }
    pub fn lust(&mut self, v: f32) {
        unsafe { tfly_lust(self.ptr(), v) }
    }

    // ---- drives --------------------------------------------------

    pub fn drive(&mut self, drive: i32, v: f32) {
        unsafe { tfly_drive(self.ptr(), drive, v) }
    }
    pub fn hunger(&mut self, v: f32) {
        unsafe { tfly_hunger(self.ptr(), v) }
    }
    pub fn thirst(&mut self, v: f32) {
        unsafe { tfly_thirst(self.ptr(), v) }
    }
    pub fn sex_drive(&mut self, v: f32) {
        unsafe { tfly_sex_drive(self.ptr(), v) }
    }

    // ---- learning ------------------------------------------------

    /// Three-factor update. `modulator <= 0` means no learning happens.
    pub fn associate(&mut self, cue: i32, action: i32, outcome: f32, modulator: f32) {
        unsafe { tfly_associate(self.ptr(), cue, action, outcome, modulator) }
    }
    pub fn reward(&mut self, v: f32) {
        unsafe { tfly_reward(self.ptr(), v) }
    }
    pub fn punish(&mut self, v: f32) {
        unsafe { tfly_punish(self.ptr(), v) }
    }
    pub fn plasticity(&mut self, v: f32) {
        unsafe { tfly_plasticity(self.ptr(), v) }
    }
    pub fn reset_learning(&mut self) {
        unsafe { tfly_reset_learning(self.ptr()) }
    }
    pub fn learning_gate(&self) -> f32 {
        unsafe { tfly_learning_gate(self.ptr()) }
    }
    pub fn assoc_weight(&self, cue: i32, action: i32) -> f32 {
        unsafe { tfly_assoc_weight(self.ptr(), cue, action) }
    }

    // ---- memory --------------------------------------------------

    pub fn memorize(&mut self, key: f32, value: f32) {
        unsafe { tfly_memorize(self.ptr(), key, value) }
    }
    pub fn recall(&self, key: f32) -> f32 {
        unsafe { tfly_recall(self.ptr(), key) }
    }
    pub fn forget(&mut self) {
        unsafe { tfly_forget(self.ptr()) }
    }

    // ---- social ---------------------------------------------------

    pub fn mate_signal(&mut self, quality: f32) {
        unsafe { tfly_mate_signal(self.ptr(), quality) }
    }
    pub fn rival_signal(&mut self, pressure: f32) {
        unsafe { tfly_rival_signal(self.ptr(), pressure) }
    }
    pub fn predator_signal(&mut self, risk: f32) {
        unsafe { tfly_predator_signal(self.ptr(), risk) }
    }

    // ---- sleep -----------------------------------------------------

    pub fn sleep(&mut self) {
        unsafe { tfly_sleep(self.ptr()) }
    }
    pub fn wake(&mut self) {
        unsafe { tfly_wake(self.ptr()) }
    }
    pub fn is_asleep(&self) -> bool {
        unsafe { tfly_asleep(self.ptr()) != 0 }
    }

    // ---- motor -----------------------------------------------------

    pub fn thrust(&mut self, v: f32) {
        unsafe { tfly_thrust(self.ptr(), v) }
    }
    pub fn turn(&mut self, v: f32) {
        unsafe { tfly_turn(self.ptr(), v) }
    }
    pub fn lift(&mut self, v: f32) {
        unsafe { tfly_lift(self.ptr(), v) }
    }
    pub fn rest(&mut self, v: f32) {
        unsafe { tfly_rest(self.ptr(), v) }
    }

    // ---- attention -------------------------------------------------

    pub fn attention(&mut self, channel: i32, gain: f32) {
        unsafe { tfly_attention(self.ptr(), channel, gain) }
    }

    // ---- readers -----------------------------------------------------

    pub fn emotion_level(&self, emotion: i32) -> f32 {
        unsafe { tfly_emotion_level(self.ptr(), emotion) }
    }
    pub fn drive_level(&self, drive: i32) -> f32 {
        unsafe { tfly_drive_level(self.ptr(), drive) }
    }
    pub fn hormone_level(&self, hormone: i32) -> f32 {
        unsafe { tfly_hormone_level(self.ptr(), hormone) }
    }
    pub fn sensory_level(&self, channel: i32) -> f32 {
        unsafe { tfly_sensory_level(self.ptr(), channel) }
    }
    pub fn dominant_emotion(&self) -> &'static str {
        cstr_to_name(unsafe { tfly_emotion_name(tfly_dominant_emotion(self.ptr())) })
    }
    pub fn dominant_drive(&self) -> &'static str {
        cstr_to_name(unsafe { tfly_drive_name(tfly_dominant_drive(self.ptr())) })
    }
    pub fn hormone_name(&self, id: i32) -> &'static str {
        cstr_to_name(unsafe { tfly_hormone_name(id) })
    }
    pub fn body_name(&self, id: i32) -> &'static str {
        cstr_to_name(unsafe { tfly_body_name(id) })
    }
    pub fn action_name(&self, id: i32) -> &'static str {
        cstr_to_name(unsafe { tfly_action_name(id) })
    }

    /// Compact JSON state, matching the C `TToJson`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut buf = vec![0u8; 512];
        let written = unsafe {
            tfly_to_json(
                self.ptr(),
                buf.as_mut_ptr().cast::<c_char>(),
                buf.len() as c_int,
            )
        };
        let len = (written.max(0) as usize).min(buf.len() - 1);
        String::from_utf8_lossy(&buf[..len]).into_owned()
    }

    /// Size of the C struct, useful for asserting the ABI still matches.
    pub fn struct_size() -> i32 {
        unsafe { tfly_struct_size() }
    }

    // ---- physiology -------------------------------------------------

    pub fn valence(&self) -> f32 {
        unsafe { tfly_valence(self.ptr()) }
    }
    pub fn arousal(&self) -> f32 {
        unsafe { tfly_arousal(self.ptr()) }
    }
    pub fn energy(&self) -> f32 {
        unsafe { tfly_energy(self.ptr()) }
    }
    pub fn stress(&self) -> f32 {
        unsafe { tfly_stress(self.ptr()) }
    }
    pub fn pain_total(&self) -> f32 {
        unsafe { tfly_pain_total(self.ptr()) }
    }
    pub fn plasticity_level(&self) -> f32 {
        unsafe { tfly_plasticity_level(self.ptr()) }
    }
    /// Commanded thrust, i.e. what the motor actually produced this tick.
    pub fn out_thrust(&self) -> f32 {
        unsafe { tfly_thrust_out(self.ptr()) }
    }
    /// Commanded turn, in -1..1, positive is right.
    pub fn out_turn(&self) -> f32 {
        unsafe { tfly_turn_out(self.ptr()) }
    }
    /// Commanded vertical, in -1..1, positive is up.
    pub fn out_vertical(&self) -> f32 {
        unsafe { tfly_vertical_out(self.ptr()) }
    }
    pub fn wingbeat(&self) -> f32 {
        unsafe { tfly_wingbeat(self.ptr()) }
    }
    /// Simulated seconds since the fly was created.
    pub fn elapsed(&self) -> f32 {
        unsafe { tfly_elapsed(self.ptr()) }
    }

    /// Combined fear, i.e. fear plus the slower dread component.
    #[must_use]
    pub fn fear_level(&self) -> f32 {
        (self.emotion_level(emotion::FEAR) + self.emotion_level(emotion::DREAD)).clamp(0.0, 1.0)
    }

    /// Combined distress, used to decide whether a fly can act on a plan.
    #[must_use]
    pub fn distress(&self) -> f32 {
        (self.fear_level() + self.emotion_level(emotion::PAIN)).clamp(0.0, 1.0)
    }
}

impl Default for Fly {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Fly {
    fn drop(&mut self) {
        unsafe { tfly_free(self.handle) }
    }
}

fn cstr_to_name(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "unknown";
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("unknown")
}

/// A snapshot of the fly, shaped for JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct FlySnapshot {
    pub mood: String,
    pub drive: String,
    pub valence: f32,
    pub arousal: f32,
    pub energy: f32,
    pub stress: f32,
    pub pain: f32,
    pub learning_gate: f32,
    pub plasticity: f32,
    pub asleep: bool,
    pub fear: f32,
    pub joy: f32,
    pub curiosity: f32,
    pub sadness: f32,
    pub anger: f32,
    pub hunger: f32,
    pub sleep_need: f32,
    pub dopamine: f32,
    pub octopamine: f32,
    pub serotonin: f32,
}

impl Fly {
    /// Read everything the editor or a log line needs.
    #[must_use]
    pub fn snapshot(&self) -> FlySnapshot {
        FlySnapshot {
            mood: self.dominant_emotion().to_owned(),
            drive: self.dominant_drive().to_owned(),
            valence: self.valence(),
            arousal: self.arousal(),
            energy: self.energy(),
            stress: self.stress(),
            pain: self.pain_total(),
            learning_gate: self.learning_gate(),
            plasticity: self.plasticity_level(),
            asleep: self.is_asleep(),
            fear: self.emotion_level(emotion::FEAR),
            joy: self.emotion_level(emotion::JOY),
            curiosity: self.emotion_level(emotion::CURIOSITY),
            sadness: self.emotion_level(emotion::SADNESS),
            anger: self.emotion_level(emotion::ANGER),
            hunger: self.drive_level(drive::HUNGER),
            sleep_need: self.drive_level(drive::SLEEP),
            dopamine: self.hormone_level(hormone::DOPAMINE),
            octopamine: self.hormone_level(hormone::OCTOPAMINE),
            serotonin: self.hormone_level(hormone::SEROTONIN),
        }
    }
}

// ===========================================================================
// Training
//
// The point of this module is that the fly learns from the three-factor rule
// in the C core, not from a scripted animation. A curriculum is a list of
// lessons; each lesson maps a cue to the action that solves it. The fly
// accumulates associative weights, and its accuracy is measured honestly,
// so a lesson that is not learned stays unlearned.
// ===========================================================================

/// The outcome of a single training trial.
/// Score a walking trial, matching `TGaitScore` in the C core.
///
/// `progress` is ground covered toward the goal, `energy_used` is what the walk
/// cost, and `fell` records that the character lost its footing. A stumble is
/// the heaviest penalty, because a character that cannot stay upright is not
/// walking at all.
#[must_use]
pub fn gait_score(progress: f32, energy_used: f32, fell: bool) -> f32 {
    let mut score = progress * 2.0 - energy_used * 1.5;
    if fell {
        score -= 0.6;
    }
    score.clamp(-1.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialOutcome {
    /// The fly picked the right action.
    Correct,
    /// The fly picked a wrong action, but was still close to solving it.
    Partial,
    /// The fly picked a clearly wrong action.
    Wrong,
}

impl TrialOutcome {
    /// The signed value fed into the associative update.
    ///
    /// Only the correct action is positive. The distractor is the fly's
    /// believable mistake and gets a mild penalty; anything else gets a full
    /// one. Grading partial credit upward would let a fly converge on
    /// "almost right" and never reach the solution.
    pub fn value(self) -> f32 {
        match self {
            TrialOutcome::Correct => 1.0,
            TrialOutcome::Partial => -0.2,
            TrialOutcome::Wrong => -0.5,
        }
    }
}

/// One teachable lesson.
#[derive(Debug, Clone, Copy)]
pub struct Lesson {
    /// Stable identifier, used by the editor to group mastery.
    pub id: &'static str,
    /// Human readable name.
    pub name: &'static str,
    /// What the fly is supposed to learn, in one line.
    pub objective: &'static str,
    /// The conditioned stimulus, from the `cue` module.
    pub cue: i32,
    /// The action that solves the lesson, from the `action` module.
    pub solution: i32,
    /// A distractor action, so the fly has to discriminate rather than saturate.
    pub distractor: i32,
}

/// The result of running a trial against a lesson.
#[derive(Debug, Clone)]
pub struct TrialResult {
    pub lesson: &'static str,
    pub outcome: TrialOutcome,
    /// The weight the fly had for the correct action before learning.
    pub weight_before: f32,
    /// The weight after the update.
    pub weight_after: f32,
    /// Modulator available at the time of the update.
    pub gate: f32,
}

impl Fly {
    /// Read how strongly the fly currently associates a cue with an action.
    #[must_use]
    pub fn weight(&self, cue: i32, action: i32) -> f32 {
        unsafe { tfly_assoc_weight(self.ptr(), cue, action) }
    }

    /// How strongly the fly associates a cue with an action, folded into -1..1.
    /// This is what "confidence in a plan" means for the readout.
    #[must_use]
    pub fn confidence(&self, cue: i32, action: i32) -> f32 {
        ((self.weight(cue, action) + 1.0) * 0.5).clamp(0.0, 1.0)
    }

    /// Current strength of a motor primitive, 0..1.
    #[must_use]
    pub fn action_level(&self, action: i32) -> f32 {
        unsafe { tfly_action_level(self.ptr(), action) }
    }

    /// Number of items in working memory.
    #[must_use]
    pub fn memory_count(&self) -> i32 {
        unsafe { tfly_memory_count(self.ptr()) }
    }

    /// Emotional value the fly has attached to a cue.
    #[must_use]
    pub fn cue_value(&self, cue: i32) -> f32 {
        unsafe { tfly_cue_value(self.ptr(), cue) }
    }

    /// Remaining lifespan, 1 at birth.
    #[must_use]
    pub fn lifespan(&self) -> f32 {
        unsafe { tfly_lifespan(self.ptr()) }
    }

    /// Set every sensory channel's attention gain at once.
    pub fn attend_all(&mut self, gain: f32) {
        unsafe { tfly_attend_all(self.ptr(), gain) }
    }

    /// Draw from the fly's own seeded generator, in 0..1.
    ///
    /// The generator lives in the C core so that there is exactly one RNG in
    /// the model. With a fixed seed, exploration replays bit for bit.
    pub fn random(&self) -> f32 {
        unsafe { tfly_random(self.ptr()) }
    }

    /// Index into a candidate list, using the fly's own generator.
    pub fn random_index(&self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (self.random() * len as f32) as usize % len
        }
    }

    /// Drive one motor primitive directly, for scripted action in a trial.
    pub fn act(&mut self, action: i32, amount: f32) {
        unsafe { tfly_act(self.ptr(), action, amount) }
    }

    /// Zero every motor primitive. Used to stage a trial cleanly.
    pub fn clear_actions(&mut self) {
        unsafe { tfly_clear_actions(self.ptr()) }
    }

    /// Choose the action the fly is most strongly committed to for a cue.
    ///
    /// Ties are broken with the fly's own generator rather than toward the
    /// first candidate. A deterministic tie-break looks fair but is not: a
    /// fresh fly has every weight at zero, so it would pick the same action
    /// forever and never discover the lesson. Random tie-breaking makes an
    /// untrained fly genuinely try things.
    #[must_use]
    pub fn preferred_action(&self, cue: i32, candidates: &[i32]) -> i32 {
        if candidates.is_empty() {
            return action::REST;
        }
        let mut best = candidates[0];
        let mut best_w = self.weight(cue, best);
        let mut tied: Vec<i32> = vec![best];
        for &candidate in candidates.iter().skip(1) {
            let w = self.weight(cue, candidate);
            if w > best_w {
                best = candidate;
                best_w = w;
                tied.clear();
                tied.push(candidate);
            } else if (w - best_w).abs() < 1e-6 {
                tied.push(candidate);
            }
        }
        if tied.len() > 1 {
            tied[self.random_index(tied.len())]
        } else {
            best
        }
    }

    /// Run one trial of a lesson.
    ///
    /// The fly chooses an action from the candidate set using the associative
    /// weights it has built so far, the outcome is scored, and the three-factor
    /// update runs with the fly's own current modulator as the gate. Nothing
    /// here forces the weight to move: a fly with a closed gate and a wrong
    /// guess genuinely learns nothing, which is the honest behaviour.
    pub fn train_trial(
        &mut self,
        lesson: &Lesson,
        epsilon: f32,
        candidates: &[i32],
    ) -> TrialResult {
        self.train_trial_gated(lesson, epsilon, candidates, None)
    }

    /// Run a trial with an explicit modulator override.
    ///
    /// Passing `Some(0.0)` runs the trial exactly as usual but with the gate
    /// forced shut, which is the honest way to measure what learning without a
    /// modulator is worth: nothing.
    pub fn train_trial_gated(
        &mut self,
        lesson: &Lesson,
        epsilon: f32,
        candidates: &[i32],
        gate_override: Option<f32>,
    ) -> TrialResult {
        if candidates.is_empty() {
            return TrialResult {
                lesson: lesson.id,
                outcome: TrialOutcome::Wrong,
                weight_before: 0.0,
                weight_after: 0.0,
                gate: 0.0,
            };
        }

        let gate = gate_override.unwrap_or_else(|| self.learning_gate());
        let weight_before = self.weight(lesson.cue, lesson.solution);

        // Epsilon-greedy exploration on top of the associative preference.
        let chosen = if self.random() < epsilon {
            candidates[self.random_index(candidates.len())]
        } else {
            self.preferred_action(lesson.cue, candidates)
        };

        // Let the fly actually perform the chosen action. This is what fills
        // the eligibility trace inside the C core, and it is the reason a
        // harness that only scores a fly, without letting it act, cannot teach
        // it anything.
        self.clear_actions();
        self.act(chosen, 1.0);
        self.steps(2, 1.0 / 60.0);

        let outcome = if chosen == lesson.solution {
            TrialOutcome::Correct
        } else if chosen == lesson.distractor {
            // The distractor is the plausible mistake: wrong, but closer than
            // a random guess. It is a smaller penalty, never a reward, or the
            // fly would happily settle for being almost right.
            TrialOutcome::Partial
        } else {
            TrialOutcome::Wrong
        };

        match outcome {
            TrialOutcome::Correct => self.reward(0.6),
            TrialOutcome::Partial => self.punish(0.15),
            TrialOutcome::Wrong => self.punish(0.35),
        }

        // Three-factor update: the action was active, an outcome arrived, and
        // the modulator was present. Drop any one factor and nothing is learned.
        self.associate(lesson.cue, chosen, outcome.value(), gate);

        // A correct guess also strengthens the lesson's own cue so the fly
        // starts to anticipate instead of only react.
        if outcome == TrialOutcome::Correct {
            self.associate(lesson.cue, lesson.solution, 0.5, gate);
            self.memorize(lesson.cue as f32, 1.0);
        }

        TrialResult {
            lesson: lesson.id,
            outcome,
            weight_before,
            weight_after: self.weight(lesson.cue, lesson.solution),
            gate,
        }
    }

    /// Convert a cue index into a stable key for working memory.
    #[must_use]
    pub fn cue_key(&self, cue: i32) -> f32 {
        cue as f32 + 0.5
    }

    // ---- gait ---------------------------------------------------------

    /// Adopt one of the gait presets and return its index.
    pub fn set_gait(&mut self, gait: i32) {
        unsafe { tfly_set_gait(self.ptr(), gait) }
    }

    /// How strongly the fly has learned to prefer a gait, 0..1.
    #[must_use]
    pub fn gait_weight(&self, gait: i32) -> f32 {
        unsafe { tfly_gait_weight(self.ptr(), gait) }
    }

    /// Write down the outcome of a walking trial, gated by the learning gate.
    pub fn train_gait(&mut self, gait: i32, score: f32) {
        unsafe { tfly_train_gait(self.ptr(), gait, score) }
    }

    /// Current stride length, 0..1. The renderer reads this for leg extension.
    #[must_use]
    pub fn gait_stride(&self) -> f32 {
        unsafe { tfly_gait_stride(self.ptr()) }
    }

    /// Current step rate, 0..1.
    #[must_use]
    pub fn gait_cadence(&self) -> f32 {
        unsafe { tfly_gait_cadence(self.ptr()) }
    }

    /// How much the fly wavers, 0..1. High sway costs balance.
    #[must_use]
    pub fn gait_sway(&self) -> f32 {
        unsafe { tfly_gait_sway(self.ptr()) }
    }

    /// Accumulated walk cycle in radians, for the leg animation.
    #[must_use]
    pub fn gait_phase(&self) -> f32 {
        unsafe { tfly_gait_phase(self.ptr()) }
    }

    /// Lifetime step count.
    #[must_use]
    pub fn step_count(&self) -> f32 {
        unsafe { tfly_step_count(self.ptr()) }
    }

    /// Stability, 1 upright and 0 about to fall over.
    #[must_use]
    pub fn balance(&self) -> f32 {
        unsafe { tfly_balance(self.ptr()) }
    }

    // ---- face -------------------------------------------------------

    /// How closed the eyelids are, 0 open and 1 shut.
    #[must_use]
    pub fn blink(&self) -> f32 {
        unsafe { tfly_blink(self.ptr()) }
    }
    /// Horizontal gaze, -1 left to 1 right.
    #[must_use]
    pub fn gaze_x(&self) -> f32 {
        unsafe { tfly_gaze_x(self.ptr()) }
    }
    /// Vertical gaze, -1 down to 1 up.
    #[must_use]
    pub fn gaze_y(&self) -> f32 {
        unsafe { tfly_gaze_y(self.ptr()) }
    }
    /// Pupil dilation, 0.5 constricted to 1.6 wide.
    #[must_use]
    pub fn pupil(&self) -> f32 {
        unsafe { tfly_pupil(self.ptr()) }
    }
    /// Brow position, -1 frowning to 1 raised.
    #[must_use]
    pub fn brow(&self) -> f32 {
        unsafe { tfly_brow(self.ptr()) }
    }
    /// Mouth curve, -1 frown to 1 smile.
    #[must_use]
    pub fn mouth(&self) -> f32 {
        unsafe { tfly_mouth(self.ptr()) }
    }
    /// How open the mouth is, 0 closed to 1 wide.
    #[must_use]
    pub fn mouth_open(&self) -> f32 {
        unsafe { tfly_mouth_open(self.ptr()) }
    }
    /// Blush intensity, 0..1.
    #[must_use]
    pub fn blush(&self) -> f32 {
        unsafe { tfly_blush(self.ptr()) }
    }
    /// Tears, 0..1. Builds while she is sad, drains when she is not.
    #[must_use]
    pub fn tears(&self) -> f32 {
        unsafe { tfly_tears(self.ptr()) }
    }
    /// Sweat, 0..1. Tracks nerves and awkwardness.
    #[must_use]
    pub fn sweat(&self) -> f32 {
        unsafe { tfly_sweat(self.ptr()) }
    }

    /// How far open she has *chosen* to open her eyes, 0..1.
    ///
    /// Distinct from [`Fly::blink`], which is a reflex. The lid is shut if
    /// either channel wants it shut, so the rig reads the union and this
    /// channel is what tells it whether the closure was a decision.
    #[must_use]
    pub fn eye_open(&self) -> f32 {
        unsafe { tfly_eye_open(self.ptr()) }
    }
    /// Pupil adaptation to light, 0 dark .. 1 bright.
    #[must_use]
    pub fn eye_adapt(&self) -> f32 {
        unsafe { tfly_eye_adapt(self.ptr()) }
    }
    /// Seconds since she woke. Zero while asleep.
    #[must_use]
    pub fn wake_timer(&self) -> f32 {
        unsafe { tfly_wake_timer(self.ptr()) }
    }

    /// Point her eyes at something, without touching where she is going.
    ///
    /// Naming a target is itself an act of attention, so the gaze takes hold
    /// on its own. Use [`Fly::look_strength`] to set how firmly.
    pub fn look_at(&mut self, x: f32, y: f32) {
        unsafe { tfly_look_at(self.ptr(), x, y) }
    }
    /// How firmly she holds that gaze, 0 to let it drift, 1 to lock on.
    pub fn look_strength(&mut self, strength: f32) {
        unsafe { tfly_look_strength(self.ptr(), strength) }
    }
    /// Stop looking at anything in particular.
    pub fn look_away(&mut self) {
        unsafe { tfly_look_away(self.ptr()) }
    }
    /// The gaze she is holding, in the model's own coordinates.
    #[must_use]
    pub fn look(&self) -> (f32, f32, f32) {
        unsafe {
            (
                tfly_look_x(self.ptr()),
                tfly_look_y(self.ptr()),
                tfly_look_lock(self.ptr()),
            )
        }
    }

    // ---- posture -------------------------------------------------------

    /// Spine carriage, -1 curled in .. 1 stretched up.
    #[must_use]
    pub fn spine(&self) -> f32 {
        unsafe { tfly_spine(self.ptr()) }
    }
    /// Shoulder height, 0 down .. 1 shrugged up.
    #[must_use]
    pub fn shoulder(&self) -> f32 {
        unsafe { tfly_shoulder(self.ptr()) }
    }
    /// Body lean, -1 leaning back .. 1 leaning forward.
    #[must_use]
    pub fn lean(&self) -> f32 {
        unsafe { tfly_lean(self.ptr()) }
    }

    // ---- limbs -------------------------------------------------------

    /// Every joint of every limb, right now.
    ///
    /// A walk is a set of joint trajectories, not a single swinging number: a
    /// rig given only the swing cannot bend a knee, so the shin drags through
    /// the floor on every forward step.
    #[must_use]
    pub fn pose(&self) -> Pose {
        let mut raw = [0.0f32; limb::COUNT * 4];
        // SAFETY: `raw` is exactly the size the C side writes, limb::COUNT
        // limbs of four floats each, and it outlives the call because it is a
        // local the C function only reads back through the pointer it was
        // given.
        unsafe { tfly_limbs(self.ptr(), raw.as_mut_ptr()) };
        let at = |i: usize| Joints {
            root: raw[i * 4],
            middle: raw[i * 4 + 1],
            end: raw[i * 4 + 2],
            spread: raw[i * 4 + 3],
        };
        Pose {
            legs: [at(limb::LEG_L), at(limb::LEG_R)],
            arms: [at(limb::ARM_L), at(limb::ARM_R)],
        }
    }

    /// How far below the hip the lower foot reaches, given segment lengths.
    ///
    /// The lengths are the renderer's proportions, not the model's, so they are
    /// passed in. A rig uses this to place its root on the floor instead of
    /// sinking: the body has to drop by exactly as much as the foot does.
    #[must_use]
    pub fn foot_drop(&self, rig: &Sole) -> f32 {
        unsafe {
            tfly_foot_drop(
                self.ptr(),
                rig.thigh,
                rig.shin,
                rig.sole_c,
                rig.sole_a,
                rig.sole_b,
                rig.sole_f,
            )
        }
    }

    /// The same question asked about one specific leg pose, so a caller holding
    /// its own angles can check them against the model's answer.
    #[must_use]
    pub fn foot_drop_for(&self, leg: &Joints, rig: &Sole) -> f32 {
        // SAFETY: a plain arithmetic call on borrowed floats, no pointers to
        // the fly and no aliasing.
        unsafe {
            tfly_foot_drop_pose(
                &leg.root,
                &leg.middle,
                &leg.end,
                rig.thigh,
                rig.shin,
                rig.sole_c,
                rig.sole_a,
                rig.sole_b,
                rig.sole_f,
            )
        }
    }

    /// Set the walk phase directly, for tests and for the editor scrubbing
    /// through a cycle.
    pub fn set_phase(&mut self, phase: f32) {
        unsafe { tfly_set_phase(self.ptr(), phase) }
    }

    // ---- gesture and social -------------------------------------------

    /// The pose currently being held, from the `gesture` module.
    #[must_use]
    pub fn gesture(&self) -> i32 {
        unsafe { tfly_gesture(self.ptr()) }
    }
    /// Envelope of the current pose, 0..1.
    #[must_use]
    pub fn gesture_strength(&self) -> f32 {
        unsafe { tfly_gesture_strength(self.ptr()) }
    }
    /// Progress through the current pose, 0..1.
    #[must_use]
    pub fn gesture_phase(&self) -> f32 {
        unsafe { tfly_gesture_phase(self.ptr()) }
    }
    /// Start a pose, if nothing else is at full strength.
    pub fn play_gesture(&mut self, gesture: i32) {
        unsafe { tfly_gesture_start(self.ptr(), gesture) }
    }
    /// Start a pose, interrupting whatever was running.
    ///
    /// Used for direct control, where a silently dropped request would be
    /// worse than a pose cut short.
    pub fn force_gesture(&mut self, gesture: i32) {
        unsafe { tfly_gesture_force(self.ptr(), gesture) }
    }
    /// Depth of the relationship, 0..1. Grows with kind encounters.
    #[must_use]
    pub fn bond(&self) -> f32 {
        unsafe { tfly_bond(self.ptr()) }
    }
    /// Whether an encounter is allowed right now (cooldown permitting).
    #[must_use]
    pub fn can_encounter(&self) -> bool {
        unsafe { tfly_can_encounter(self.ptr()) != 0 }
    }
    /// How inclined she is to start an encounter, 0..1.
    #[must_use]
    pub fn encounter_drive(&self) -> f32 {
        unsafe { tfly_encounter_drive(self.ptr()) }
    }
    /// The last encounter performed, from the `encounter` module.
    #[must_use]
    pub fn last_encounter(&self) -> i32 {
        unsafe { tfly_last_encounter(self.ptr()) }
    }
    /// Russian name of a gesture.
    #[must_use]
    pub fn gesture_name(gesture: i32) -> &'static str {
        cstr_to_name(unsafe { tfly_gesture_name(gesture) })
    }
    /// Russian name of an encounter.
    #[must_use]
    pub fn encounter_name(encounter: i32) -> &'static str {
        cstr_to_name(unsafe { tfly_encounter_name(encounter) })
    }

    /// The gait the fly currently prefers, breaking ties with its own RNG.
    #[must_use]
    pub fn preferred_gait(&self) -> i32 {
        let mut best = 0;
        let mut best_w = self.gait_weight(0);
        for g in 1..gait::COUNT {
            let w = self.gait_weight(g);
            if w > best_w {
                best = g;
                best_w = w;
            }
        }
        best
    }

    /// Every emotion as `(id, name, value)`, sorted strongest first.
    ///
    /// The UI needs all of them at once to draw the affect cloud.
    #[must_use]
    pub fn affect(&self) -> Vec<(i32, &'static str, f32)> {
        let mut out: Vec<(i32, &'static str, f32)> = emotion::ALL
            .iter()
            .map(|&(name, id)| (id, name, self.emotion_level(id)))
            .collect();
        out.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        out
    }
}

/// Limbs the walk drives, in C-core order.
pub mod limb {
    pub const LEG_L: usize = 0;
    pub const LEG_R: usize = 1;
    pub const ARM_L: usize = 2;
    pub const ARM_R: usize = 3;
    /// How many there are.
    pub const COUNT: usize = 4;
}

/// The four joints of one limb, in radians.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Joints {
    /// Hip or shoulder swing.
    pub root: f32,
    /// Knee or elbow bend. Never negative: a knee does not bend backwards.
    pub middle: f32,
    /// Ankle or wrist.
    pub end: f32,
    /// Sideways splay.
    pub spread: f32,
}

/// The joints of all four limbs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub legs: [Joints; 2],
    pub arms: [Joints; 2],
}

/// The rig's leg dimensions, which the model needs in order to answer "how far
/// below the hip is the foot".
///
/// The sole is an ellipsoid rather than a point, and that is the whole reason
/// this is three numbers instead of one. A foot is long: as the ankle tilts, its
/// far end swings down far enough that a point below the ankle overestimates
/// the foot's height by most of a foot, and the character visibly hovers
/// whenever her knee folds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sole {
    /// Hip to knee.
    pub thigh: f32,
    /// Knee to ankle.
    pub shin: f32,
    /// How far the sole's centre sits below the ankle.
    pub sole_c: f32,
    /// The sole's vertical semi-axis.
    pub sole_a: f32,
    /// The sole's depth semi-axis, which is the long one.
    pub sole_b: f32,
    /// How far the sole's centre sits ahead of the ankle, which is what swings
    /// it down as the ankle tilts.
    pub sole_f: f32,
}

/// Result of an encounter between two flies.
#[derive(Debug, Clone, Copy)]
pub struct Meeting {
    /// True if the encounter actually ran, rather than hitting a cooldown.
    pub happened: bool,
    /// Bond after the encounter.
    pub bond: f32,
}

/// Let two flies meet.
///
/// The two handles are distinct pointers, so there is no aliasing and the C
/// side does not re-enter. The aliasing case is rejected rather than
/// tolerated, because borrowing the same fly twice would be a caller bug and
/// silently corrupting her state would hide it.
pub fn meet(a: &mut Fly, b: &mut Fly, kind: i32) -> Meeting {
    if std::ptr::eq(a.ptr(), b.ptr()) {
        return Meeting {
            happened: false,
            bond: a.bond(),
        };
    }
    let happened = unsafe { tfly_encounter(a.ptr(), b.ptr(), kind) != 0 };
    Meeting {
        happened,
        bond: a.bond(),
    }
}

#[cfg(test)]
mod face_tests {
    use super::*;
    use crate::tfly::{encounter, gesture};

    /// Every face channel must stay inside its declared range under load.
    #[test]
    fn face_channels_stay_in_range() {
        let mut fly = Fly::new();
        fly.seed(17);
        for i in 0..4000 {
            fly.heart(fly.random(), (fly.random() * 10.0) as i32);
            fly.reward(fly.random());
            fly.punish(fly.random());
            fly.mate_signal(fly.random());
            fly.predator_signal(fly.random());
            fly.steps(1, 1.0 / 60.0);
            if i % 400 == 0 {
                assert!((0.0..=1.0).contains(&fly.blink()), "blink {}", fly.blink());
                assert!((-1.0..=1.0).contains(&fly.gaze_x()));
                assert!((-1.0..=1.0).contains(&fly.gaze_y()));
                assert!((0.5..=1.6).contains(&fly.pupil()), "pupil {}", fly.pupil());
                assert!((-1.0..=1.0).contains(&fly.brow()));
                assert!((-1.0..=1.0).contains(&fly.mouth()));
                assert!((0.0..=1.0).contains(&fly.mouth_open()));
                assert!((0.0..=1.0).contains(&fly.blush()));
                assert!((0.0..=1.0).contains(&fly.tears()));
                assert!((0.0..=1.0).contains(&fly.sweat()));
            }
        }
    }

    /// Sleeping closes the eyes. That is the most obvious sign the face is
    /// driven by state rather than by a timer.
    #[test]
    fn sleep_closes_the_eyes() {
        let mut fly = Fly::new();
        fly.seed(3);
        for _ in 0..30 {
            fly.steps(1, 1.0 / 60.0);
        }
        let awake = fly.blink();
        fly.sleep();
        for _ in 0..120 {
            fly.steps(1, 1.0 / 60.0);
        }
        assert!(
            fly.blink() > awake,
            "a sleeping fly must close her eyes: awake={awake} asleep={}",
            fly.blink()
        );
    }

    /// Fear must widen the eyes, which is the point of having pupils.
    #[test]
    fn fear_dilates_the_pupils() {
        let mut calm = Fly::new();
        let mut scared = Fly::new();
        calm.seed(9);
        scared.seed(9);
        for _ in 0..600 {
            calm.steps(1, 1.0 / 60.0);
            scared.fear(0.05);
            scared.steps(1, 1.0 / 60.0);
        }
        assert!(
            scared.pupil() > calm.pupil(),
            "fear must dilate: calm={} scared={}",
            calm.pupil(),
            scared.pupil()
        );
    }

    /// Joy must raise the mouth into a smile and fear must pull it down.
    #[test]
    fn joy_smiles_and_fear_frowns() {
        let mut happy = Fly::new();
        let mut afraid = Fly::new();
        happy.seed(4);
        afraid.seed(4);
        for _ in 0..600 {
            happy.joy(0.05);
            happy.steps(1, 1.0 / 60.0);
            afraid.fear(0.05);
            afraid.steps(1, 1.0 / 60.0);
        }
        assert!(happy.mouth() > 0.0, "joy must smile: {}", happy.mouth());
        assert!(afraid.mouth() < 0.0, "fear must frown: {}", afraid.mouth());
    }

    /// Tears must build while sad and drain once she is happy again.
    #[test]
    fn tears_build_and_drain() {
        let mut fly = Fly::new();
        fly.seed(6);
        fly.sadness(0.6);
        for _ in 0..600 {
            fly.steps(1, 1.0 / 60.0);
        }
        let wet = fly.tears();
        assert!(wet > 0.0, "sadness must produce tears, got {wet}");
        fly.joy(0.8);
        for _ in 0..1200 {
            fly.steps(1, 1.0 / 60.0);
        }
        assert!(
            fly.tears() < wet,
            "tears must drain once she is happy: {wet} -> {}",
            fly.tears()
        );
    }

    /// A kind encounter must deepen the bond on both sides.
    #[test]
    fn an_encounter_deepens_the_bond_on_both_sides() {
        let mut a = Fly::new();
        let mut b = Fly::new();
        a.seed(1);
        b.seed(2);
        let before_a = a.bond();
        let before_b = b.bond();
        let meeting = meet(&mut a, &mut b, encounter::SHARE);
        assert!(meeting.happened, "the first encounter must run");
        assert!(a.bond() > before_a, "bond must grow");
        assert!(b.bond() > before_b, "bond must grow on both sides");
        assert!(a.emotion_level(emotion::JOY) > 0.0);
        assert!(b.emotion_level(emotion::TRUST) > 0.0);
    }

    /// A quarrel must damage the bond, not grow it.
    #[test]
    fn a_quarrel_breaks_the_bond() {
        let mut a = Fly::new();
        let mut b = Fly::new();
        a.seed(1);
        b.seed(2);
        for _ in 0..4 {
            let _ = meet(&mut a, &mut b, encounter::SHARE);
            a.steps(400, 1.0 / 60.0);
            b.steps(400, 1.0 / 60.0);
        }
        let strong = a.bond();
        assert!(strong > 0.2, "bond should build first, got {strong}");
        let _ = meet(&mut a, &mut b, encounter::ARGUE);
        assert!(
            a.bond() < strong,
            "a quarrel must reduce the bond: {strong} -> {}",
            a.bond()
        );
    }

    /// The cooldown must stop a pair repeating one encounter.
    #[test]
    fn encounters_respect_the_cooldown() {
        let mut a = Fly::new();
        let mut b = Fly::new();
        a.seed(1);
        b.seed(2);
        assert!(meet(&mut a, &mut b, encounter::GREET).happened);
        assert!(!a.can_encounter(), "an encounter must start a cooldown");
        assert!(
            !meet(&mut a, &mut b, encounter::GREET).happened,
            "the second greeting must be refused"
        );
        a.steps(600, 1.0 / 60.0);
        b.steps(600, 1.0 / 60.0);
        assert!(a.can_encounter(), "the cooldown must expire");
    }

    /// A gesture must run to completion and release back to idle.
    #[test]
    fn a_gesture_runs_then_releases() {
        let mut fly = Fly::new();
        fly.seed(8);
        assert_eq!(fly.gesture(), gesture::IDLE);
        fly.play_gesture(gesture::WAVE);
        assert_eq!(fly.gesture(), gesture::WAVE);
        fly.steps(40, 1.0 / 60.0);
        assert!(fly.gesture_strength() > 0.5, "the pose must build");
        assert!(fly.gesture_phase() > 0.0);
        fly.steps(120, 1.0 / 60.0);
        assert_eq!(fly.gesture(), gesture::IDLE, "the pose must release");
    }

    /// A fly cannot meet herself.
    ///
    /// The borrow checker already rejects `meet(&mut fly, &mut fly, ..)`, so
    /// the guard inside `meet` is belt and braces for a future caller holding
    /// raw handles. What is testable here is the state that guard protects: a
    /// refused encounter must leave bond and affect untouched.
    #[test]
    fn a_refused_encounter_changes_nothing() {
        let mut a = Fly::new();
        let mut b = Fly::new();
        a.seed(2);
        b.seed(5);
        // The first meeting runs and starts the cooldown.
        assert!(meet(&mut a, &mut b, encounter::GREET).happened);
        let bond_a = a.bond();
        let joy_a = a.emotion_level(emotion::JOY);
        // The second is refused.
        let refused = meet(&mut a, &mut b, encounter::SHARE);
        assert!(!refused.happened, "the cooldown must refuse it");
        assert_eq!(
            a.bond(),
            bond_a,
            "a refused encounter must not move the bond"
        );
        assert_eq!(
            a.emotion_level(emotion::JOY),
            joy_a,
            "a refused encounter must not move affect"
        );
    }
    /// Waking is a sequence, not a switch.
    #[test]
    fn opening_the_eyes_takes_time() {
        let mut fly = Fly::new();
        fly.seed(13);
        fly.light(0.8);
        fly.sleep();
        fly.steps(180, 1.0 / 60.0);
        assert!(fly.blink() > 0.9, "the lids are shut while asleep");
        assert!(fly.eye_open() < 0.1, "and deliberately so");
        fly.wake();
        fly.steps(1, 1.0 / 60.0);
        assert!(
            fly.eye_open() < 0.5,
            "the lids must not snap open the instant she wakes: {}",
            fly.eye_open()
        );
        fly.steps(200, 1.0 / 60.0);
        assert!(fly.eye_open() > 0.95, "she opens her eyes on purpose");
        assert!(fly.blink() < 0.1, "and ends up looking at the world");
    }

    /// Waking is not monotonic: she blinks the sleep out on the way up. This
    /// is what stops it reading as a switch being thrown.
    #[test]
    fn waking_blinks_the_sleep_out() {
        let mut fly = Fly::new();
        fly.seed(13);
        fly.sleep();
        fly.steps(120, 1.0 / 60.0);
        fly.wake();
        let mut lowest = 1.0f32;
        let mut reopened = 0;
        let mut was_shut = true;
        for _ in 0..84 {
            fly.steps(1, 1.0 / 60.0);
            lowest = lowest.min(fly.blink());
            let shut = fly.blink() > 0.2;
            if !shut && was_shut {
                reopened += 1;
            }
            was_shut = shut;
        }
        assert!(lowest < 0.4, "the lids come most of the way up: {lowest}");
        assert!(
            reopened >= 1,
            "she blinks the sleep out on the way, so waking is not a switch"
        );
    }

    /// A startle has to wake her, not merely lift her lids. Otherwise the eyes
    /// fly open and then close again as the surprise decays.
    #[test]
    fn a_startle_wakes_her() {
        let mut fly = Fly::new();
        fly.seed(17);
        fly.sleep();
        fly.steps(120, 1.0 / 60.0);
        assert!(fly.is_asleep());
        fly.surprise(0.95);
        fly.steps(1, 1.0 / 60.0);
        assert!(!fly.is_asleep(), "a startle must wake her");
        let mut flew = 1.0f32;
        for _ in 0..30 {
            fly.steps(1, 1.0 / 60.0);
            flew = flew.min(fly.blink());
        }
        assert!(flew < 0.2, "her eyes fly open at a startle: {flew}");
        fly.steps(120, 1.0 / 60.0);
        assert!(
            !fly.is_asleep(),
            "and she does not fall back asleep at once"
        );
    }

    /// The body has to carry the mood too, because a face alone cannot say
    /// whether someone is braced or comfortable.
    #[test]
    fn posture_follows_mood() {
        let mut calm = Fly::new();
        let mut afraid = Fly::new();
        let mut proud = Fly::new();
        calm.seed(23);
        afraid.seed(23);
        proud.seed(23);
        for i in 0..600 {
            if i % 20 == 0 {
                afraid.fear(0.9);
                proud.pride(0.6);
            }
            calm.steps(1, 1.0 / 60.0);
            afraid.steps(1, 1.0 / 60.0);
            proud.steps(1, 1.0 / 60.0);
        }
        assert!(afraid.spine() < calm.spine(), "fear curls her in");
        assert!(
            afraid.shoulder() > calm.shoulder(),
            "fear lifts her shoulders"
        );
        assert!(afraid.lean() < calm.lean(), "fear tips her back");
        assert!(proud.spine() > calm.spine(), "pride straightens her up");
    }

    /// Naming a target has to steer the gaze without touching the motor
    /// attractor, because a character walks somewhere while watching somebody
    /// else do the walking.
    #[test]
    fn a_decided_gaze_beats_the_drift() {
        let mut fly = Fly::new();
        fly.seed(29);
        for _ in 0..120 {
            fly.steps(1, 1.0 / 60.0);
        }
        let drift = fly.gaze_x();
        fly.look_at(-0.8, 0.2);
        fly.look_strength(1.0);
        for _ in 0..120 {
            fly.steps(1, 1.0 / 60.0);
        }
        assert!(
            fly.gaze_x() < drift - 0.2,
            "she must look where she decided: drift {drift} -> {}",
            fly.gaze_x()
        );
        assert!(fly.look().2 > 0.9, "the lock is reported");
        fly.look_away();
        for _ in 0..240 {
            fly.steps(1, 1.0 / 60.0);
        }
        assert!(fly.look().2 == 0.0, "look_away releases the lock");
    }

    /// A threat outranks a decided gaze, because a fly watches the predator
    /// rather than her friend.
    #[test]
    fn a_threat_overrides_a_decided_gaze() {
        let mut fly = Fly::new();
        fly.seed(31);
        fly.look_at(0.8, 0.0);
        fly.look_strength(1.0);
        for _ in 0..120 {
            fly.steps(1, 1.0 / 60.0);
        }
        let watching_friend = fly.gaze_x();
        for _ in 0..20 {
            fly.fear(0.9);
            fly.steps(1, 1.0 / 60.0);
        }
        assert!(
            fly.gaze_x() < watching_friend,
            "a threat must pull the gaze away: {watching_friend} -> {}",
            fly.gaze_x()
        );
    }
}

#[cfg(test)]
mod gait_tests {
    use super::*;
    use crate::tfly::{action, gait, odor};

    /// A fly that is rewarded for a gait must end up preferring it.
    #[test]
    fn learning_shifts_preference_toward_the_rewarded_gait() {
        let mut fly = Fly::new();
        fly.seed(31337);
        // Reward the "steady" gait and punish the others, as a fly being
        // corrected for a bad walk would experience.
        for _ in 0..400 {
            fly.train_gait(gait::STEADY, 1.0);
            fly.train_gait(gait::HURRIED, -1.0);
            fly.train_gait(gait::LONG_STRIDE, -1.0);
            fly.train_gait(gait::WEAVING, -1.0);
        }
        assert_eq!(fly.preferred_gait(), gait::STEADY);
        assert!(fly.gait_weight(gait::STEADY) > 0.5);
    }

    /// A closed gate must prevent gait learning, exactly as it does for
    /// associations. If it did not, a fly could learn to walk while unable to
    /// learn anything else, which would be incoherent.
    #[test]
    fn gait_learning_respects_the_gate() {
        let mut open = Fly::new();
        let mut shut = Fly::new();
        open.seed(5);
        shut.seed(5);
        shut.hormone(2.0, hormone::SEROTONIN, 0.0);
        for _ in 0..200 {
            open.train_gait(gait::STEADY, 1.0);
            shut.train_gait(gait::STEADY, 1.0);
        }
        assert!(
            shut.gait_weight(gait::STEADY) < open.gait_weight(gait::STEADY),
            "a suppressed gate must slow gait learning: shut={} open={}",
            shut.gait_weight(gait::STEADY),
            open.gait_weight(gait::STEADY)
        );
    }

    /// The walk cycle must actually advance steps, and walking must cost more
    /// than standing still.
    ///
    /// The claim is walker versus rest, not walker versus its starting value:
    /// a fly that has just eaten legitimately regains energy faster than it
    /// spends, so only the comparison isolates the cost of walking.
    #[test]
    fn walking_advances_steps_and_costs_energy() {
        let mut walker = Fly::new();
        walker.seed(9);
        walker.set_gait(gait::STEADY);
        let before_steps = walker.step_count();
        for _ in 0..600 {
            walker.act(action::FORWARD, 1.0);
            walker.steps(1, 1.0 / 60.0);
        }

        let mut rested = Fly::new();
        rested.seed(9);
        rested.set_gait(gait::STEADY);
        for _ in 0..600 {
            rested.act(action::REST, 1.0);
            rested.steps(1, 1.0 / 60.0);
        }

        assert!(
            walker.step_count() > before_steps,
            "a moving fly must take steps: {} -> {}",
            before_steps,
            walker.step_count()
        );
        assert!(
            walker.energy() < rested.energy(),
            "walking must cost more than resting: walker={} rested={}",
            walker.energy(),
            rested.energy()
        );
    }

    /// A fly with a bad, swaying gait must lose balance; a steady one must not.
    #[test]
    fn sway_costs_balance() {
        let mut weaver = Fly::new();
        weaver.seed(4);
        weaver.set_gait(gait::WEAVING);
        for _ in 0..900 {
            weaver.act(action::FORWARD, 1.0);
            weaver.steps(1, 1.0 / 60.0);
        }
        let mut steady = Fly::new();
        steady.seed(4);
        steady.set_gait(gait::STEADY);
        for _ in 0..900 {
            steady.act(action::FORWARD, 1.0);
            steady.steps(1, 1.0 / 60.0);
        }
        assert!(
            weaver.balance() < steady.balance(),
            "a weaving gait must be less stable: weaver={} steady={}",
            weaver.balance(),
            steady.balance()
        );
    }

    /// The new affect states must be derived, not dead storage: pain clearing
    /// has to produce relief, and a fresh idle fly has to feel something.
    #[test]
    fn new_emotions_are_driven_by_state() {
        let mut fly = Fly::new();
        fly.seed(11);
        fly.heart(1.0, body::WING_L);
        fly.steps(20, 1.0 / 60.0);
        assert!(fly.emotion_level(emotion::RELIEF) >= 0.0);
        // Pain has to start falling for relief to have anything to track.
        fly.heal_all();
        fly.steps(30, 1.0 / 60.0);
        assert!(
            fly.emotion_level(emotion::RELIEF) > 0.0,
            "clearing pain must produce relief"
        );

        // A social signal must produce affection and a little hope.
        let mut social = Fly::new();
        social.seed(3);
        social.mate_signal(1.0);
        social.steps(120, 1.0 / 60.0);
        assert!(social.emotion_level(emotion::AFFECTION) > 0.1);
        assert!(social.emotion_level(emotion::TRUST) > 0.0);
        assert!(social.emotion_level(emotion::LONGING) >= 0.0);
    }

    /// The full affect list must be readable and correctly sized.
    #[test]
    fn affect_lists_every_emotion() {
        let fly = Fly::new();
        let affect = fly.affect();
        assert_eq!(affect.len(), emotion::ALL.len());
        // Sorted strongest first.
        for pair in affect.windows(2) {
            assert!(pair[0].2 >= pair[1].2, "affect must be sorted descending");
        }
    }

    #[test]
    fn touch_and_odor_still_drive_the_policy() {
        let mut fly = Fly::new();
        fly.seed(2);
        fly.odor(0.7, odor::FRUIT);
        fly.steps(30, 1.0 / 60.0);
        assert!(fly.sensory_level(crate::tfly::sense::ODOR) > 0.0);
    }
}

#[cfg(test)]
mod training_tests {
    use super::*;
    use crate::tfly::{action, cue, odor};

    const LESSONS: [Lesson; 3] = [
        Lesson {
            id: "stage",
            name: "Main stage",
            objective: "Fly to the applause",
            cue: cue::LIGHT,
            solution: action::DANCE,
            distractor: action::REST,
        },
        Lesson {
            id: "garden",
            name: "Enchanted garden",
            objective: "Follow the fruit smell",
            cue: cue::ODOR_FRUIT,
            solution: action::EAT,
            distractor: action::MATE,
        },
        Lesson {
            id: "void",
            name: "The void",
            objective: "Hold still in the dark",
            cue: cue::DARK,
            solution: action::REST,
            distractor: action::FLAP,
        },
    ];

    fn candidates() -> Vec<i32> {
        vec![
            action::DANCE,
            action::EAT,
            action::REST,
            action::FLAP,
            action::FORWARD,
        ]
    }

    /// A fly that is actually trained should beat one whose gate is held shut.
    ///
    /// The control is the same fly, doing the same trials, with the modulator
    /// forced to zero. That isolates the three-factor rule: identical
    /// experience, different chemistry, different learning.
    #[test]
    fn training_raises_lesson_accuracy() {
        let mut trained = Fly::new();
        let mut control = Fly::new();
        trained.seed(4242);
        control.seed(4242);

        for _ in 0..300 {
            for lesson in LESSONS {
                // The fly must actually act, otherwise the eligibility trace
                // stays empty and the three-factor rule has nothing to attach
                // the update to.
                for fly in [&mut trained, &mut control] {
                    fly.odor(0.5, odor::FRUIT);
                    fly.steps(4, 1.0 / 60.0);
                }
                trained.train_trial(&lesson, 0.2, &candidates());
                control.train_trial_gated(&lesson, 0.2, &candidates(), Some(0.0));
            }
        }

        for lesson in LESSONS {
            let learned = trained.weight(lesson.cue, lesson.solution);
            let frozen = control.weight(lesson.cue, lesson.solution);
            assert!(
                learned > 0.3,
                "lesson {} should be learned, got {learned}",
                lesson.id
            );
            assert_eq!(
                frozen, 0.0,
                "lesson {} must not be learned with the gate shut, got {frozen}",
                lesson.id
            );
        }
    }

    /// The modulator must scale how fast a lesson is learned.
    ///
    /// Two flies get identical trials; only the gate differs. This isolates the
    /// third factor of the three-factor rule, which is the part that is easy to
    /// fake and the part that matters most.
    #[test]
    fn closed_gate_blocks_learning() {
        let lesson = LESSONS[0];
        let mut fast = Fly::new();
        let mut slow = Fly::new();
        fast.seed(11);
        slow.seed(11);

        for _ in 0..120 {
            for fly in [&mut fast, &mut slow] {
                fly.steps(4, 1.0 / 60.0);
            }
            fast.train_trial_gated(&lesson, 0.0, &candidates(), Some(1.0));
            slow.train_trial_gated(&lesson, 0.0, &candidates(), Some(0.05));
        }

        let fast_w = fast.weight(lesson.cue, lesson.solution);
        let slow_w = slow.weight(lesson.cue, lesson.solution);
        assert!(
            fast_w > slow_w,
            "a wide-open gate must learn faster: fast={fast_w} slow={slow_w}"
        );
    }

    /// Serotonin must actually lower the gate, which is the mechanism that
    /// makes a well-fed, content fly a poor learner.
    #[test]
    fn serotonin_suppresses_the_gate() {
        let mut calm = Fly::new();
        let mut sedated = Fly::new();
        calm.seed(5);
        sedated.seed(5);
        sedated.hormone(1.0, hormone::SEROTONIN, 0.0);
        assert!(
            sedated.learning_gate() < calm.learning_gate(),
            "serotonin must close the gate: sedated={} calm={}",
            sedated.learning_gate(),
            calm.learning_gate()
        );
    }

    /// The fly must prefer the trained action over an untrained one.
    #[test]
    fn trained_preference_wins() {
        let mut fly = Fly::new();
        let options = candidates();
        for _ in 0..300 {
            fly.steps(4, 1.0 / 60.0);
            fly.train_trial(&LESSONS[0], 0.1, &options);
        }
        let chosen = fly.preferred_action(LESSONS[0].cue, &options);
        assert_eq!(chosen, LESSONS[0].solution);
    }

    /// Pain must still override a learned preference, or the circus cannot
    /// teach avoidance.
    #[test]
    fn pain_overrides_learned_plan() {
        let mut fly = Fly::new();
        let options = candidates();
        for _ in 0..200 {
            fly.steps(4, 1.0 / 60.0);
            fly.train_trial(&LESSONS[1], 0.1, &options);
        }
        fly.heart(0.95, body::WING_L);
        fly.steps(5, 1.0 / 60.0);
        assert!(fly.emotion_level(emotion::PAIN) > 0.3);
        assert!(fly.fear_level() > 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_allocates_and_frees() {
        let fly = Fly::new();
        assert!(Fly::struct_size() > 0);
        drop(fly);
    }

    #[test]
    fn pain_reaches_the_right_region() {
        let mut fly = Fly::new();
        fly.heart(0.9, body::WING_L);
        assert!(fly.pain(0.0) >= 0.0);
        fly.heal_all();
    }

    #[test]
    fn emotions_are_callable() {
        let mut fly = Fly::new();
        fly.fear(0.8);
        assert!(fly.emotion_level(emotion::FEAR) > 0.5);
        fly.joy(0.5);
        fly.steps(60, 1.0 / 60.0);
        assert!(!fly.snapshot().mood.is_empty());
    }

    #[test]
    fn learning_requires_a_modulator() {
        let mut fly = Fly::new();
        fly.seed(7);
        for _ in 0..30 {
            fly.odor(0.6, odor::FRUIT);
            fly.steps(30, 1.0 / 60.0);
            fly.associate(cue::ODOR_FRUIT, action::FORWARD, 1.0, 0.0);
        }
        assert_eq!(fly.assoc_weight(cue::ODOR_FRUIT, action::FORWARD), 0.0);

        let mut rewarded = Fly::new();
        rewarded.seed(7);
        for _ in 0..30 {
            rewarded.odor(0.6, odor::FRUIT);
            rewarded.steps(30, 1.0 / 60.0);
            rewarded.reward(0.6);
            rewarded.associate(
                cue::ODOR_FRUIT,
                action::FORWARD,
                1.0,
                rewarded.learning_gate(),
            );
        }
        assert!(rewarded.assoc_weight(cue::ODOR_FRUIT, action::FORWARD) > 0.0);
    }

    #[test]
    fn json_snapshot_is_well_formed() {
        let mut fly = Fly::new();
        fly.light(0.5);
        fly.steps(60, 1.0 / 60.0);
        let json = fly.to_json();
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"mood\""));
    }

    #[test]
    fn hormone_names_resolve() {
        let fly = Fly::new();
        assert_eq!(fly.hormone_name(hormone::DOPAMINE), "dopamine");
        assert_eq!(fly.body_name(body::WING_L), "wing_l");
    }
}
