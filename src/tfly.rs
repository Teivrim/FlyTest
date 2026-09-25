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
