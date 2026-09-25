/*
 * tfly_ffi.c - external-linkage shim over TFLY.h
 *
 * TFLY.h is a header-only library, so its functions are `static inline` and
 * have no symbols the linker can see. This file includes the header once and
 * exports a small, stable C ABI so the Rust runtime can drive a fly without
 * duplicating any of the model logic.
 *
 * Every function here is a thin wrapper. No behaviour lives in this file.
 */

#include "../TFLY.h"

#include <stdlib.h>

/* Opaque handle type. TFLY is the real struct; this alias exists so the
 * exported API reads as a handle and never leaks the layout. */
typedef TFLY TFlyHandle;

TFlyHandle *tfly_new(void) {
    TFlyHandle *handle = (TFlyHandle *)calloc(1, sizeof(TFlyHandle));
    if (handle == NULL) return NULL;
    TNew(handle);
    return handle;
}

void tfly_free(TFlyHandle *handle) { free(handle); }

void tfly_seed(TFlyHandle *h, unsigned seed) { TSeed(h, seed); }

void tfly_update(TFlyHandle *h, float dt) { TUpdate(h, dt); }
void tfly_steps(TFlyHandle *h, int n, float dt) { TSteps(h, n, dt); }

/* sensory */
void tfly_light(TFlyHandle *h, float v) { TLight(h, v); }
void tfly_odor(TFlyHandle *h, float v, int kind) { TOdor(h, v, kind); }
void tfly_touch(TFlyHandle *h, float v) { TTouch(h, v); }
void tfly_temperature(TFlyHandle *h, float v) { TTemperature(h, v); }
void tfly_sound(TFlyHandle *h, float v) { TSound(h, v); }
void tfly_vibration(TFlyHandle *h, float v) { TVibration(h, v); }
void tfly_taste(TFlyHandle *h, float sweet, float salt, float bitter, float water) {
    TTaste(h, sweet, salt, bitter, water);
}

/* nociception */
float tfly_heart(TFlyHandle *h, float intensity, int region) { return THeart(h, intensity, region); }
float tfly_pain(TFlyHandle *h, float v) { return TPain(h, v); }
void tfly_heal(TFlyHandle *h, int region, float rate) { THeal(h, region, rate); }
void tfly_heal_all(TFlyHandle *h) { THealAll(h); }

/* chemistry */
float tfly_hormone(TFlyHandle *h, float intensity, int hormone, float duration) {
    return THormone(h, intensity, hormone, duration);
}
float tfly_hormone_named(TFlyHandle *h, float intensity, const char *name, float duration) {
    return THormoneByName(h, intensity, name, duration);
}
void tfly_clear_hormones(TFlyHandle *h) { TClearHormones(h); }

/* affect */
void tfly_emotion(TFlyHandle *h, int emotion, float v) { TEmotion(h, emotion, v); }
void tfly_fear(TFlyHandle *h, float v) { TFear(h, v); }
void tfly_joy(TFlyHandle *h, float v) { TJoy(h, v); }
void tfly_sadness(TFlyHandle *h, float v) { TSadness(h, v); }
void tfly_anger(TFlyHandle *h, float v) { TAnger(h, v); }
void tfly_curiosity(TFlyHandle *h, float v) { TCuriosity(h, v); }
void tfly_contentment(TFlyHandle *h, float v) { TContentment(h, v); }
void tfly_dread(TFlyHandle *h, float v) { TDread(h, v); }
void tfly_lust(TFlyHandle *h, float v) { TLust(h, v); }

/* drives */
void tfly_drive(TFlyHandle *h, int drive, float v) { TDrive(h, drive, v); }
void tfly_hunger(TFlyHandle *h, float v) { THunger(h, v); }
void tfly_thirst(TFlyHandle *h, float v) { TThirst(h, v); }
void tfly_sex_drive(TFlyHandle *h, float v) { TSexDrive(h, v); }

/* learning */
void tfly_associate(TFlyHandle *h, int cue, int action, float outcome, float modulator) {
    TAssociate(h, cue, action, outcome, modulator);
}
void tfly_reward(TFlyHandle *h, float v) { TReward(h, v); }
void tfly_punish(TFlyHandle *h, float v) { TPunish(h, v); }
void tfly_plasticity(TFlyHandle *h, float v) { TPlasticity(h, v); }
void tfly_reset_learning(TFlyHandle *h) { TResetLearning(h); }
float tfly_learning_gate(TFlyHandle *h) { return TLearningGate(h); }
float tfly_assoc_weight(TFlyHandle *h, int cue, int action) { return h->assoc[TClampIdx(cue, TFLY_N_CUE)][TClampIdx(action, TFLY_N_ACTION)]; }

/* memory */
void tfly_memorize(TFlyHandle *h, float key, float value) { TMemorize(h, key, value); }
float tfly_recall(TFlyHandle *h, float key) { return TRecall(h, key); }
void tfly_forget(TFlyHandle *h) { TForget(h); }

/* social */
void tfly_mate_signal(TFlyHandle *h, float q) { TMateSignal(h, q); }
void tfly_rival_signal(TFlyHandle *h, float p) { TRivalSignal(h, p); }
void tfly_predator_signal(TFlyHandle *h, float r) { TPredatorSignal(h, r); }

/* sleep */
void tfly_sleep(TFlyHandle *h) { TSleep(h); }
void tfly_wake(TFlyHandle *h) { TWake(h); }

/* motor */
void tfly_thrust(TFlyHandle *h, float v) { TThrust(h, v); }
void tfly_turn(TFlyHandle *h, float v) { TTurn(h, v); }
void tfly_lift(TFlyHandle *h, float v) { TLift(h, v); }
void tfly_rest(TFlyHandle *h, float v) { TRest(h, v); }

/* attention */
void tfly_attention(TFlyHandle *h, int channel, float gain) { TAttention(h, channel, gain); }

/* readers */
float tfly_emotion_level(TFlyHandle *h, int e) { return TEmotionLevel(h, e); }
float tfly_drive_level(TFlyHandle *h, int d) { return TDriveLevel(h, d); }
float tfly_hormone_level(TFlyHandle *h, int x) { return THormoneLevel(h, x); }
float tfly_sensory_level(TFlyHandle *h, int s) { return TSensoryLevel(h, s); }
int tfly_dominant_emotion(TFlyHandle *h) { return TDominantEmotion(h); }
int tfly_dominant_drive(TFlyHandle *h) { return TDominantDrive(h); }
int tfly_asleep(TFlyHandle *h) { return h->asleep; }

/* scalar physiology, needed for the editor's per-tick readout */
float tfly_valence(TFlyHandle *h) { return h->valence; }
float tfly_arousal(TFlyHandle *h) { return h->arousal; }
float tfly_energy(TFlyHandle *h) { return h->energy; }
float tfly_stress(TFlyHandle *h) { return h->stress; }
float tfly_pain_total(TFlyHandle *h) { return h->pain_total; }
float tfly_plasticity_level(TFlyHandle *h) { return h->plasticity; }
float tfly_thrust_out(TFlyHandle *h) { return h->out_thrust; }
float tfly_turn_out(TFlyHandle *h) { return h->out_turn; }
float tfly_vertical_out(TFlyHandle *h) { return h->out_vertical; }
float tfly_wingbeat(TFlyHandle *h) { return h->out_wingbeat; }
float tfly_elapsed(TFlyHandle *h) { return h->t; }

int tfly_to_json(TFlyHandle *h, char *buf, int cap) { return TToJson(h, buf, cap); }

const char *tfly_hormone_name(int id) { return TFlyHormoneName(id); }
const char *tfly_emotion_name(int id) { return TFlyEmotionName(id); }
const char *tfly_drive_name(int id) { return TFlyDriveName(id); }
const char *tfly_action_name(int id) { return TFlyActionName(id); }
const char *tfly_body_name(int id) { return TFlyBodyName(id); }

/* size of the opaque struct, so Rust can allocate it correctly */
int tfly_struct_size(void) { return (int)sizeof(TFLY); }
