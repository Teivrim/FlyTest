/*
 * TFLY.h - single-header fly brain emulation library.
 *
 * Design goals
 * ------------
 *  1. Single header. Add this file to your translation unit and you are done:
 *         #include "TFLY.h"
 *     No .c file, no build system, no linking. Requires C99 and libm.
 *
 *  2. Explicit wiring, no magic. Every internal variable is a plain field on
 *     the `TFLY` struct. You can inspect it, clamp it, script it, or blow it
 *     up. Nothing is hidden behind a black box.
 *
 *  3. Bilingual naming on purpose. The API uses the `T` prefix the project
 *     already uses for stimuli (TLight, TOdor, ...). Reading side gets
 *     `TFly*` names so that a stimulus and its reader never collide.
 *
 *  4. Honest neuroscience. The three-factor learning rule (CS x US x modulator)
 *     in TAssociate() mirrors how insect olfactory conditioning actually works.
 *     The rest is an engineering approximation, not a claim about real brains.
 *
 * Quick start
 * -----------
 *     TFLY fly;
 *     TNew(&fly);
 *
 *     for (int i = 0; i < 600; i++) {
 *         TLight(&fly, 0.6f);
 *         TOdor(&fly, 0.5f, T_ODOR_FRUIT);
 *         TTick(&fly, 1.0f / 60.0f);
 *     }
 *
 *     TPrintState(&fly);
 *
 * Threading: a TFLY instance is not thread safe. Use one instance per fly, or
 * guard it externally.
 *
 * License: same as the rest of the repository.
 */

#ifndef TFLY_H
#define TFLY_H

#include <math.h>
#include <stdio.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

#define TFLY_VERSION_MAJOR 1
#define TFLY_VERSION_MINOR 0

/* ------------------------------------------------------------------ *
 * Size limits
 * ------------------------------------------------------------------ */

#define TFLY_N_SENSORY 12
#define TFLY_N_REGION 10
#define TFLY_N_EMOTION 26
#define TFLY_N_DRIVE 8
#define TFLY_N_HORMONE 26
#define TFLY_N_ACTION 10
#define TFLY_N_CUE 12
#define TFLY_N_MEMORY 24
#define TFLY_N_PULSE 8
/* Gait presets a fly can learn to walk with. */
#define TFLY_N_GAIT 4
/* Named gestures the rig can pose. */
#define TFLY_N_GESTURE 8
/* Kinds of encounter between two flies. */
#define TFLY_N_ENCOUNTER 7
/* How long a gesture takes, in seconds. */
#define GESTURE_SECONDS 1.2f

/* ------------------------------------------------------------------ *
 * Sensory channels
 * ------------------------------------------------------------------ */

enum {
    T_LIGHT = 0,      /* ambient photon level, 0..1            */
    T_DARK,           /* inverse of T_LIGHT, computed          */
    T_ODOR,           /* olfactory channel, 0..1              */
    T_TOUCH,          /* mechanosensory, 0..1                  */
    T_TEMPERATURE,    /* normalised, 0.5 == preferred          */
    T_GRAVITY,        /* 0..1 == 1g down                       */
    T_PROPRIOCEPTION, /* body angle / load feedback           */
    T_SOUND,          /* acoustic, 0..1                        */
    T_VIBRATION,      /* substrate vibration, 0..1             */
    T_TASTE,          /* summed taste quality, 0..1            */
    T_SMELL_ANTENNA,  /* antennal lobe gain, 0..1              */
    T_WIND,           /* relative airspeed, 0..1               */
    T_SENSE_END
};

/* ------------------------------------------------------------------ *
 * Body regions
 * ------------------------------------------------------------------ */

enum {
    T_BODY_HEAD = 0,
    T_BODY_THORAX,
    T_BODY_ABDOMEN,
    T_BODY_WING_L,
    T_BODY_WING_R,
    T_BODY_LEG_L,
    T_BODY_LEG_R,
    T_BODY_EYE_L,
    T_BODY_EYE_R,
    T_BODY_PROBOSCIS,
    T_REGION_END
};

/* ------------------------------------------------------------------ *
 * Emotions. These are affective variables, not statements about
 * subjective experience. See docs/tfly-design.md for the argument.
 * ------------------------------------------------------------------ */

enum {
    T_EMO_PAIN = 0,
    T_EMO_FEAR,
    T_EMO_JOY,
    T_EMO_SADNESS,
    T_EMO_ANGER,
    T_EMO_DISGUST,
    T_EMO_SURPRISE,
    T_EMO_CURIOSITY,
    T_EMO_LUST,
    T_EMO_CRAVING,
    T_EMO_CONTENTMENT,
    T_EMO_DREAD,
    T_EMO_CONFUSION,
    T_EMO_PRIDE,
    /* The richer half. These are what make a character read as a person
     * rather than as a state vector: they are social, anticipatory, and they
     * decay at very different rates from the reflexes above. */
    T_EMO_GRATITUDE,
    T_EMO_RELIEF,
    T_EMO_DISAPPOINTMENT,
    T_EMO_HOPE,
    T_EMO_JEALOUSY,
    T_EMO_SHYNESS,
    T_EMO_AFFECTION,
    T_EMO_BOREDOM,
    T_EMO_EXCITEMENT,
    T_EMO_COMPASSION,
    T_EMO_TRUST,
    T_EMO_LONGING,
    T_EMOTION_END
};

/* ------------------------------------------------------------------ *
 * Drives (homeostatic set-point errors, 0 = satisfied, 1 = starving)
 * ------------------------------------------------------------------ */

enum {
    T_DRIVE_HUNGER = 0,
    T_DRIVE_THIRST,
    T_DRIVE_SEX,
    T_DRIVE_SLEEP,
    T_DRIVE_FATIGUE,
    T_DRIVE_COLD,
    T_DRIVE_SOCIAL,
    T_DRIVE_CURIOSITY,
    T_DRIVE_END
};

/* ------------------------------------------------------------------ *
 * Hormones. Names follow the actual Drosophila peptidergic system
 * where a real analogue exists.
 * ------------------------------------------------------------------ */

enum {
    T_H_OCTOPAMINE = 0,
    T_H_DOPAMINE,
    T_H_SEROTONIN,
    T_H_NOVELTY,
    T_H_INSULIN,
    T_H_LEPTIN,
    T_H_ECDYSONE,
    T_H_JUVENILE_HORMONE,
    T_H_GLUTAMATE,
    T_H_GABA,
    T_H_ACETYLCHOLINE,
    T_H_NITRIC_OXIDE,
    T_H_CORAZONIN,
    T_H_NEUROPEPTIDE_F,
    T_H_DH31,
    T_H_FMRFAMIDE,
    T_H_CCAP,
    T_H_RELAXIN,
    T_H_ETH,
    T_H_IRS,
    T_H_TOR,
    T_H_SNPF,
    T_H_NLPP,
    T_H_PAM,
    T_H_PTTH,
    T_H_DH44,
    T_HORMONE_END
};

/* ------------------------------------------------------------------ *
 * Motor primitives
 * ------------------------------------------------------------------ */

enum {
    T_ACT_REST = 0,
    T_ACT_FORWARD,
    T_ACT_TURN_L,
    T_ACT_TURN_R,
    T_ACT_UP,
    T_ACT_DOWN,
    T_ACT_FLAP,
    T_ACT_DANCE,
    T_ACT_EAT,
    T_ACT_MATE,
    T_ACTION_END
};

/* ------------------------------------------------------------------ *
 * Associative cues (conditioned stimuli)
 * ------------------------------------------------------------------ */

enum {
    T_CUE_LIGHT = 0,
    T_CUE_DARK,
    T_CUE_ODOR_FRUIT,
    T_CUE_ODOR_FLOWER,
    T_CUE_ODOR_FEMALE,
    T_CUE_ODOR_MALE,
    T_CUE_TOUCH,
    T_CUE_TEMPERATURE,
    T_CUE_SOUND,
    T_CUE_VIBRATION,
    T_CUE_VISUAL,
    T_CUE_GRAVITY,
    T_CUE_END
};

/* Odour identities accepted by TOdor(). */
enum { T_ODOR_FRUIT = 0, T_ODOR_FLOWER, T_ODOR_FEMALE, T_ODOR_MALE, T_ODOR_PREDATOR, T_ODOR_DECAY };

/* Gestures. The rig poses these; the model decides which one runs. */
enum {
    T_GES_IDLE = 0,
    T_GES_WAVE,      /* greeting */
    T_GES_BOW,       /* respect, apology */
    T_GES_CLAP,      /* applause, celebration */
    T_GES_POINT,     /* attention, direction */
    T_GES_COVER,     /* comfort, hands to chest */
    T_GES_SHRUG,     /* uncertainty */
    T_GES_HOLD,      /* reaching out, asking to be held */
    T_GES_END
};

/* Encounters. Each has a symmetric effect on the bond between two flies. */
enum {
    T_ENC_GREET = 0,
    T_ENC_BOW,
    T_ENC_HIGH_FIVE,
    T_ENC_COMFORT,
    T_ENC_SHARE,
    T_ENC_ARGUE,
    T_ENC_IGNORE,
    T_ENC_END
};

/* ------------------------------------------------------------------ *
 * The fly
 * ------------------------------------------------------------------ */

typedef struct {
    /* sensory input and per-channel adaptation */
    float sensory[TFLY_N_SENSORY];
    float sensory_raw[TFLY_N_SENSORY];
    float gain[TFLY_N_SENSORY];   /* attention multiplier, 0..2          */
    float adapt[TFLY_N_SENSORY];  /* slow fatigue baseline               */
    float novel[TFLY_N_SENSORY];  /* |input - baseline|, novelty signal  */

    /* nociception, per region */
    float pain[TFLY_N_REGION];
    float pain_total;
    float pain_memory;            /* lingering effect of recent injury   */

    /* affect */
    float emotion[TFLY_N_EMOTION];
    float valence;                /* -1 .. 1                            */
    float arousal;                /*  0 .. 1                            */

    /* drives */
    float drive[TFLY_N_DRIVE];

    /* chemistry */
    float hormone[TFLY_N_HORMONE];
    float pulse[TFLY_N_HORMONE];  /* released this tick                 */
    float hormone_set[TFLY_N_HORMONE]; /* tonic target, decayed to it   */

    /* associative memory: cue x action, plus eligibility traces */
    float assoc[TFLY_N_CUE][TFLY_N_ACTION];
    float elig[TFLY_N_ACTION];
    float cue_value[TFLY_N_CUE];
    float plasticity;

    /* working memory: [key, value] pairs, insertion ordered */
    float memory[TFLY_N_MEMORY][2];
    int memory_n;

    /* motor output */
    float action[TFLY_N_ACTION];
    float out_thrust, out_turn, out_vertical, out_wingbeat;

    /* Gait. These are what the fly learns: stride, cadence, sway, and how hard
     * it pulls toward a companion while walking. They approach the targets
     * computed in TPolicy, but the fly can override them by learning weights
     * through TAssociate. */
    float gait_stride;   /* step length, 0..1 */
    float gait_cadence;  /* steps per second, 0..1 */
    float gait_sway;     /* how much it wavers, 0..1 */
    float gait_approach; /* pull toward a mate, 0..1 */
    float gait_phase;   /* accumulated walk cycle, radians */
    float steps;         /* lifetime step count */
    float balance;       /* 1 stable, 0 falling over */
    unsigned gait_weight[TFLY_N_GAIT];

    /* ---- face ----
     * The rig reads these and nothing else. Keeping them in the model means
     * blinking, gaze and expression are consequences of the fly's state
     * rather than decoration painted over it. */
    float blink;      /* 1 fully closed, 0 wide open */
    float blink_rate; /* urgency of the next blink */
    float gaze_x;     /* -1 left .. 1 right */
    float gaze_y;     /* -1 down .. 1 up */
    float pupil;      /* dilation, 0.5 constricted .. 1.6 wide */
    float brow;       /* -1 frowning .. 1 raised */
    float mouth;      /* -1 frown .. 1 smile */
    float mouth_open; /* 0 closed, 1 wide open */
    float blush;      /* 0..1 */
    float tears;      /* 0..1, builds while sad */
    float sweat;      /* 0..1, builds while nervous */

    /* ---- gesture and bond ---- */
    int gesture;            /* current T_GES_* */
    float gesture_strength; /* 0..1 envelope */
    float gesture_phase;    /* 0..1 progress through the gesture */
    float bond;             /* depth of the relationship, 0..1 */
    float encounter_cool;   /* seconds until another encounter is allowed */
    int last_encounter;     /* last T_ENC_* performed */

    /* physiology */
    float energy;     /* 0..1 */
    float stress;     /* 0..1 */
    float lifespan;   /* 0..1, 1 == fresh */
    float age;        /* seconds */
    float temp_body;  /* 0..1 */

    /* world coupling */
    float odor_lateral;   /* -1 left .. +1 right bias            */
    float target_x;       /* normalised attractor, -1..1         */
    float target_y;
    float mate_quality;   /* 0..1                              */
    float rival_pressure; /* 0..1                              */
    float predator_risk;  /* 0..1                              */

    /* bookkeeping */
    unsigned rng;
    unsigned ticks;
    float t;
    int asleep;
} TFLY;

/* ------------------------------------------------------------------ *
 * Forward declarations. The header is ordered by topic rather than by
 * dependency, so these few functions are announced up front.
 * ------------------------------------------------------------------ */

static inline void TEmit(TFLY *fly, int hormone, float amount);
static inline void TResetLearning(TFLY *fly);
static inline void TRecompute(TFLY *fly);
static inline void TRecomputeValence(TFLY *fly);
static inline void TPlasticity(TFLY *fly, float rate);
static inline const char *TFlyHormoneName(int hormone);
static inline const char *TFlyEmotionName(int emotion);
static inline const char *TFlyDriveName(int drive);
static inline const char *TFlyActionName(int action);
static inline const char *TFlySensoryName(int channel);
static inline const char *TFlyBodyName(int region);
static inline int TDominantEmotion(TFLY *fly);
static inline int TDominantDrive(TFLY *fly);
static inline float TFlyEmotionHalfLife(int emotion);
static inline float TLearningGate(TFLY *fly);
static inline float TExp(float current, float target, float tau, float dt);
static inline void TTemperature(TFLY *fly, float normalised);

/* ------------------------------------------------------------------ *
 * Utilities
 * ------------------------------------------------------------------ */

static inline float TClamp(float v, float lo, float hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
}

static inline float TAbs(float v) { return v < 0.0f ? -v : v; }

static inline float TLerp(float a, float b, float t) { return a + (b - a) * t; }

/* deterministic xorshift32, so replays are bit-identical */
static inline unsigned TRandU32(TFLY *fly) {
    unsigned x = fly->rng;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    fly->rng = x;
    return x;
}

static inline float TRand(TFLY *fly) {
    return (float)(TRandU32(fly) & 0x00FFFFFFu) / 16777215.0f;
}

static inline float TGauss(TFLY *fly) {
    return (TRand(fly) + TRand(fly) + TRand(fly) + TRand(fly) - 2.0f) * 0.8164965f;
}

static inline float TApproach(float cur, float target, float rate) {
    return cur + (target - cur) * TClamp(rate, 0.0f, 1.0f);
}

static inline float TMax(float a, float b) { return a > b ? a : b; }
static inline float TMin(float a, float b) { return a < b ? a : b; }

static inline int TClampIdx(int v, int n) { return v < 0 ? 0 : (v >= n ? n - 1 : v); }

/* ------------------------------------------------------------------ *
 * Lifecycle
 * ------------------------------------------------------------------ */

/* First-order exponential approach towards a target. */
static inline float TExp(float current, float target, float tau, float dt) {
    if (tau <= 0.0f) return target;
    return current + (target - current) * (1.0f - expf(-dt / tau));
}

static inline void TNew(TFLY *fly) {
    if (fly == NULL) return;
    memset(fly, 0, sizeof(TFLY));
    fly->rng = 0x9E3779B9u;
    for (int i = 0; i < TFLY_N_SENSORY; i++) {
        fly->gain[i] = 1.0f;
        fly->sensory[T_DARK] = 1.0f;
    }
    fly->sensory[T_TEMPERATURE] = 0.5f;
    fly->sensory[T_GRAVITY] = 1.0f;
    fly->energy = 1.0f;
    fly->lifespan = 1.0f;
    fly->temp_body = 0.5f;
    fly->plasticity = 1.0f;
    fly->emotion[T_EMO_CONTENTMENT] = 0.2f;
    fly->out_wingbeat = 0.0f;
    /* A newborn fly has a usable but untrained gait. */
    fly->gait_stride = 0.45f;
    fly->gait_cadence = 0.5f;
    fly->gait_sway = 0.4f;
    fly->gait_approach = 0.0f;
    fly->balance = 1.0f;
    /* A newborn starts with open eyes, a neutral face, and no attachments. */
    fly->pupil = 0.8f;
    fly->blink = 0.0f;
    fly->blink_rate = 0.5f;
    fly->gesture = T_GES_IDLE;
    fly->last_encounter = T_ENC_IGNORE;
    TResetLearning(fly);
}

static inline void TResetLearning(TFLY *fly) {
    if (fly == NULL) return;
    for (int c = 0; c < TFLY_N_CUE; c++)
        for (int a = 0; a < TFLY_N_ACTION; a++) fly->assoc[c][a] = 0.0f;
    for (int a = 0; a < TFLY_N_ACTION; a++) fly->elig[a] = 0.0f;
    for (int c = 0; c < TFLY_N_CUE; c++) fly->cue_value[c] = 0.0f;
    for (int g = 0; g < TFLY_N_GAIT; g++) fly->gait_weight[g] = 0u;
}

/* Seed the stimulus RNG so a run can be replayed exactly. */
static inline void TSeed(TFLY *fly, unsigned seed) {
    if (fly != NULL) fly->rng = (seed != 0u) ? seed : 0x9E3779B9u;
}

/* ------------------------------------------------------------------ *
 * Sensory input
 *
 * Every setter writes into `sensory_raw`. TLight, TOdor, TTouch and friends
 * also call TRecompute to keep the derived channels consistent, so you can
 * call them in any order within a tick.
 * ------------------------------------------------------------------ */

static inline void TRecompute(TFLY *fly) {
    fly->sensory[T_DARK] = 1.0f - TClamp(fly->sensory_raw[T_LIGHT], 0.0f, 1.0f);
    fly->sensory[T_SMELL_ANTENNA] = TClamp(0.15f + 0.85f * fly->sensory_raw[T_ODOR] *
                                               fly->gain[T_ODOR],
                                           0.0f, 1.0f);
}

static inline void TLight(TFLY *fly, float intensity) {
    if (fly == NULL) return;
    fly->sensory_raw[T_LIGHT] = TClamp(intensity, 0.0f, 1.0f);
    TRecompute(fly);
}

/* Phototaxis: a signed preference, negative means "fly away from light". */
static inline void TPhototaxis(TFLY *fly, float bias) {
    if (fly == NULL) return;
    fly->target_x = TClamp(bias, -1.0f, 1.0f);
}

static inline void TOdor(TFLY *fly, float intensity, int kind) {
    if (fly == NULL) return;
    float v = TClamp(intensity, 0.0f, 1.0f) * fly->gain[T_ODOR];
    fly->sensory_raw[T_ODOR] = TClamp(fly->sensory_raw[T_ODOR] * 0.6f + v, 0.0f, 1.0f);
    TRecompute(fly);

    /* Odour identity maps onto a lateral bias and onto cue drive. */
    switch (TClampIdx(kind, 6)) {
        case T_ODOR_FRUIT:
            fly->odor_lateral += 0.10f * v;
            fly->drive[T_DRIVE_HUNGER] = TClamp(fly->drive[T_DRIVE_HUNGER] - 0.02f * v, 0.0f, 1.0f);
            break;
        case T_ODOR_FLOWER:
            fly->emotion[T_EMO_CONTENTMENT] = TClamp(fly->emotion[T_EMO_CONTENTMENT] + 0.05f * v, 0.0f, 1.0f);
            fly->drive[T_DRIVE_SOCIAL] = TClamp(fly->drive[T_DRIVE_SOCIAL] - 0.01f * v, 0.0f, 1.0f);
            break;
        case T_ODOR_FEMALE:
            fly->drive[T_DRIVE_SEX] = TClamp(fly->drive[T_DRIVE_SEX] + 0.05f * v, 0.0f, 1.0f);
            fly->emotion[T_EMO_LUST] = TClamp(fly->emotion[T_EMO_LUST] + 0.06f * v, 0.0f, 1.0f);
            break;
        case T_ODOR_MALE:
            fly->emotion[T_EMO_ANGER] = TClamp(fly->emotion[T_EMO_ANGER] + 0.05f * v, 0.0f, 1.0f);
            fly->rival_pressure = TClamp(fly->rival_pressure + 0.04f * v, 0.0f, 1.0f);
            break;
        case T_ODOR_PREDATOR:
            fly->predator_risk = TClamp(fly->predator_risk + 0.08f * v, 0.0f, 1.0f);
            break;
        default:
            break;
    }
}

/* Horizontal direction of an odour plume, -1 left .. +1 right. */
static inline void TOdorDirection(TFLY *fly, float lateral) {
    if (fly == NULL) return;
    fly->odor_lateral += TClamp(lateral, -1.0f, 1.0f) * 0.25f;
}

static inline void TTouch(TFLY *fly, float intensity) {
    if (fly == NULL) return;
    fly->sensory_raw[T_TOUCH] = TClamp(intensity, 0.0f, 1.0f);
}

static inline void TTemperature(TFLY *fly, float normalised) {
    if (fly == NULL) return;
    fly->sensory_raw[T_TEMPERATURE] = TClamp(normalised, 0.0f, 1.0f);
}

static inline void TGravity(TFLY *fly, float g) {
    if (fly == NULL) return;
    fly->sensory_raw[T_GRAVITY] = TClamp(g, 0.0f, 1.0f);
}

static inline void TProprioception(TFLY *fly, float value) {
    if (fly == NULL) return;
    fly->sensory_raw[T_PROPRIOCEPTION] = TClamp(value, -1.0f, 1.0f);
}

static inline void TSound(TFLY *fly, float intensity) {
    if (fly == NULL) return;
    fly->sensory_raw[T_SOUND] = TClamp(intensity, 0.0f, 1.0f);
}

static inline void TVibration(TFLY *fly, float intensity) {
    if (fly == NULL) return;
    fly->sensory_raw[T_VIBRATION] = TClamp(intensity, 0.0f, 1.0f);
}

static inline void TWind(TFLY *fly, float airspeed) {
    if (fly == NULL) return;
    fly->sensory_raw[T_WIND] = TClamp(airspeed, 0.0f, 1.0f);
}

/*
 * Taste is signed: bitter drives disgust, sugar drives reward. `sweet` and
 * `bitter` are 0..1, `salt` and `water` are 0..1 deficits.
 */
static inline void TTaste(TFLY *fly, float sweet, float salt, float bitter, float water) {
    if (fly == NULL) return;
    float s = TClamp(sweet, 0.0f, 1.0f);
    float sa = TClamp(salt, 0.0f, 1.0f);
    float b = TClamp(bitter, 0.0f, 1.0f);
    float q = TClamp(s - b * 0.8f - sa * 0.1f + 0.15f, 0.0f, 1.0f);
    fly->sensory_raw[T_TASTE] = q;
    fly->drive[T_DRIVE_HUNGER] = TClamp(fly->drive[T_DRIVE_HUNGER] - 0.10f * s, 0.0f, 1.0f);
    fly->drive[T_DRIVE_THIRST] = TClamp(fly->drive[T_DRIVE_THIRST] - 0.10f * TClamp(water, 0.0f, 1.0f), 0.0f, 1.0f);
    fly->emotion[T_EMO_DISGUST] = TClamp(fly->emotion[T_EMO_DISGUST] + 0.10f * b, 0.0f, 1.0f);
    fly->hormone[T_H_DOPAMINE] = TClamp(fly->hormone[T_H_DOPAMINE] + 0.15f * s, 0.0f, 1.0f);
}

/* ------------------------------------------------------------------ *
 * Nociception
 *
 * THeart(fly, x, y) is the canonical pain call:
 *     x  intensity, 0..1
 *     y  region, one of the T_BODY_* values
 *
 * Pain is not a single number. It is localised, it accumulates, it has a
 * memory that outlives the stimulus, and it rewrites behaviour immediately.
 * ------------------------------------------------------------------ */

static inline float THeart(TFLY *fly, float intensity, int region) {
    if (fly == NULL) return 0.0f;
    int r = TClampIdx(region, TFLY_N_REGION);
    float v = TClamp(intensity, 0.0f, 1.0f);

    /* local accumulation with fast onset and slow clearance */
    fly->pain[r] = TClamp(fly->pain[r] + v * 0.9f, 0.0f, 1.5f);

    /* referred spread: thorax pain is felt everywhere, wing pain stays local */
    if (r == T_BODY_THORAX) {
        for (int i = 0; i < TFLY_N_REGION; i++) fly->pain[i] = TClamp(fly->pain[i] + v * 0.15f, 0.0f, 1.5f);
    } else if (r == T_BODY_HEAD) {
        fly->pain[T_BODY_EYE_L] = TClamp(fly->pain[T_BODY_EYE_L] + v * 0.2f, 0.0f, 1.5f);
        fly->pain[T_BODY_EYE_R] = TClamp(fly->pain[T_BODY_EYE_R] + v * 0.2f, 0.0f, 1.5f);
    }

    fly->pain_total = 0.0f;
    for (int i = 0; i < TFLY_N_REGION; i++) fly->pain_total += fly->pain[i];
    fly->pain_total = TClamp(fly->pain_total * 0.25f, 0.0f, 1.5f);
    fly->pain_memory = TClamp(fly->pain_memory + v, 0.0f, 1.0f);

    /* immediate affect */
    fly->emotion[T_EMO_PAIN] = TClamp(fly->emotion[T_EMO_PAIN] + 0.7f * v, 0.0f, 1.0f);
    fly->emotion[T_EMO_DREAD] = TClamp(fly->emotion[T_EMO_DREAD] + 0.35f * v, 0.0f, 1.0f);
    fly->emotion[T_EMO_JOY] *= (1.0f - 0.6f * v);
    fly->stress = TClamp(fly->stress + 0.35f * v, 0.0f, 1.0f);

    /* stress chemistry, released this very tick */
    TEmit(fly, T_H_OCTOPAMINE, 0.30f * v);
    TEmit(fly, T_H_NEUROPEPTIDE_F, 0.10f * v);

    /* a startle reflex overrides whatever the fly intended to do */
    if (v > 0.25f) {
        fly->action[T_ACT_FLAP] = TClamp(fly->action[T_ACT_FLAP] + v, 0.0f, 1.0f);
        fly->action[T_ACT_UP] = TClamp(fly->action[T_ACT_UP] + 0.7f * v, 0.0f, 1.0f);
        fly->action[T_ACT_TURN_R] = TClamp(fly->action[T_ACT_TURN_R] + 0.5f * v, 0.0f, 1.0f);
        fly->action[T_ACT_REST] *= (1.0f - v);
    }
    return fly->pain_total;
}

/* Nociceptive channel without an explicit region. */
static inline float TPain(TFLY *fly, float intensity) { return THeart(fly, intensity, T_BODY_THORAX); }

/* Scented irritant: pain plus disgust, with no mechanical component. */
static inline float TNoxious(TFLY *fly, float intensity) {
    if (fly == NULL) return 0.0f;
    float v = TClamp(intensity, 0.0f, 1.0f);
    fly->emotion[T_EMO_DISGUST] = TClamp(fly->emotion[T_EMO_DISGUST] + 0.6f * v, 0.0f, 1.0f);
    fly->drive[T_DRIVE_CURIOSITY] = TClamp(fly->drive[T_DRIVE_CURIOSITY] - 0.3f * v, 0.0f, 1.0f);
    TEmit(fly, T_H_SEROTONIN, 0.25f * v);
    return THeart(fly, v * 0.7f, T_BODY_PROBOSCIS);
}

/* Analgesia: raises the pain threshold rather than deleting the signal. */
static inline void THeal(TFLY *fly, float region, float rate) {
    if (fly == NULL) return;
    int r = TClampIdx(region, TFLY_N_REGION);
    fly->pain[r] = TClamp(fly->pain[r] - TClamp(rate, 0.0f, 1.0f), 0.0f, 1.5f);
    fly->pain_total = 0.0f;
    for (int i = 0; i < TFLY_N_REGION; i++) fly->pain_total += fly->pain[i];
    fly->pain_total = TClamp(fly->pain_total * 0.25f, 0.0f, 1.5f);
    fly->emotion[T_EMO_PAIN] = TClamp(fly->emotion[T_EMO_PAIN] - 0.4f * rate, 0.0f, 1.0f);
}

static inline void THealAll(TFLY *fly) {
    if (fly == NULL) return;
    for (int i = 0; i < TFLY_N_REGION; i++) fly->pain[i] = 0.0f;
    fly->pain_total = 0.0f;
    fly->pain_memory = 0.0f;
    fly->emotion[T_EMO_PAIN] = 0.0f;
}

/* ------------------------------------------------------------------ *
 * Hormones
 *
 * THormone(fly, x, y, z) is the canonical call:
 *     x  intensity, 0..1
 *     y  hormone id, one of the T_H_* values
 *     z  pulse duration in seconds; <= 0 means a tonic (sustained) change
 * ------------------------------------------------------------------ */

/*
 * Release a hormone already present in the compartment. It records a pulse
 * for the current tick, which is what a downstream observer or the Blender
 * bridge reports, and adds a smaller amount to the tonic level, because a
 * release physically enters the haemolymph. Keeping the two together is what
 * lets TReward() reopen the learning gate.
 *
 * This is deliberately not the same as THormone(). Use THormone to administer
 * a hormone from outside, TEmit to model something the fly released itself.
 */
static inline void TEmit(TFLY *fly, int hormone, float amount) {
    if (fly == NULL) return;
    int h = TClampIdx(hormone, TFLY_N_HORMONE);
    float a = TClamp(amount, -1.0f, 2.0f);
    fly->pulse[h] = TClamp(fly->pulse[h] + a, 0.0f, 2.0f);
    fly->hormone[h] = TClamp(fly->hormone[h] + 0.3f * a, 0.0f, 2.0f);
}

static inline float THormone(TFLY *fly, float intensity, int hormone, float duration) {
    if (fly == NULL) return 0.0f;
    int h = TClampIdx(hormone, TFLY_N_HORMONE);
    float v = TClamp(intensity, 0.0f, 1.0f);

    /* Administration from outside raises the level by exactly the dose. */
    fly->hormone[h] = TClamp(fly->hormone[h] + v, 0.0f, 2.0f);
    if (duration > 0.0f) {
        /* A timed bolus is also reported as a pulse, but it is already
         * accounted for in the level above, so it must not be re-added. */
        fly->pulse[h] = TClamp(fly->pulse[h] + v, 0.0f, 2.0f);
    } else {
        /* tonic: shift the set-point so the level is maintained over time */
        fly->hormone_set[h] = TClamp(fly->hormone_set[h] + v, 0.0f, 2.0f);
    }
    return fly->hormone[h];
}

static inline float THormoneByName(TFLY *fly, float intensity, const char *name, float duration) {
    if (fly == NULL || name == NULL) return 0.0f;
    for (int h = 0; h < TFLY_N_HORMONE; h++) {
        if (strcmp(name, TFlyHormoneName(h)) == 0) return THormone(fly, intensity, h, duration);
    }
    return 0.0f;
}

/* Named conveniences, so callers do not have to remember the enum. */
static inline float TDopamine(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_DOPAMINE, d); }
static inline float TOctopamine(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_OCTOPAMINE, d); }
static inline float TSerotonin(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_SEROTONIN, d); }
static inline float TInsulin(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_INSULIN, d); }
static inline float TEcdysone(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_ECDYSONE, d); }
static inline float TJuvenileHormone(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_JUVENILE_HORMONE, d); }
static inline float TNeuropeptideF(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_NEUROPEPTIDE_F, d); }
static inline float TGlu(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_GLUTAMATE, d); }
static inline float TGABA(TFLY *fly, float v, float d) { return THormone(fly, v, T_H_GABA, d); }

/* Blunt every chemokine. Useful between trials. */
static inline void TClearHormones(TFLY *fly) {
    if (fly == NULL) return;
    for (int i = 0; i < TFLY_N_HORMONE; i++) {
        fly->hormone[i] = 0.0f;
        fly->hormone_set[i] = 0.0f;
        fly->pulse[i] = 0.0f;
    }
}

/* ------------------------------------------------------------------ *
 * Gait
 *
 * A fly does not pick "walk fast" out of the air; it tries one of a small set
 * of gait presets, and the three-factor rule decides which of them it keeps.
 * That is the honest way to model learning to walk: the fly cannot invent a
 * new stride, it can only get better at the ones it has tried.
 * ------------------------------------------------------------------ */

/* The four gaits a fly can learn. Each is a stride/cadence/sway triple. */
static inline void TFlyGaitPreset(int g, float *stride, float *cadence, float *sway) {
    switch (TClampIdx(g, TFLY_N_GAIT)) {
        case 0: *stride = 0.22f; *cadence = 0.85f; *sway = 0.15f; return; /* hurried */
        case 1: *stride = 0.55f; *cadence = 0.50f; *sway = 0.25f; return; /* steady */
        case 2: *stride = 0.80f; *cadence = 0.35f; *sway = 0.45f; return; /* long stride */
        default: *stride = 0.40f; *cadence = 0.60f; *sway = 0.60f; return; /* weaving */
    }
}

static inline const char *TFlyGaitName(int g) {
    switch (TClampIdx(g, TFLY_N_GAIT)) {
        case 0: return "торопливый";
        case 1: return "ровный";
        case 2: return "длинный";
        default: return "петляющий";
    }
}

/* Apply a preset to the fly's body. This is what the animation reads. */
static inline void TSetGait(TFLY *fly, int g) {
    if (fly == NULL) return;
    float s, c, w;
    TFlyGaitPreset(g, &s, &c, &w);
    fly->gait_stride = s;
    fly->gait_cadence = c;
    fly->gait_sway = w;
}

/* How strongly the fly has learned to prefer a gait, 0..1. */
static inline float TGaitWeight(TFLY *fly, int g) {
    if (fly == NULL) return 0.0f;
    return (float)fly->gait_weight[TClampIdx(g, TFLY_N_GAIT)] / 1000.0f;
}

/*
 * The outcome of a walking trial. Distance is progress in the right
 * direction, cost is energy burned, and a stumble is penalised hardest
 * because a fly that cannot stay upright is not really walking.
 */
static inline float TGaitScore(TFLY *fly, float distance, float energy_used, int fell) {
    if (fly == NULL) return 0.0f;
    float score = distance * 2.0f - energy_used * 1.5f;
    if (fell) score -= 0.6f;
    return TClamp(score, -1.0f, 1.0f);
}

/*
 * One gait trial. The fly adopts a preset, walks for `window` seconds, and
 * the caller scores how far it got. The update is the same three-factor rule
 * used everywhere else: the gait was actually performed, an outcome arrived,
 * and the modulator decided whether it could be written down at all.
 */
static inline void TTrainGait(TFLY *fly, int g, float score) {
    if (fly == NULL) return;
    int gi = TClampIdx(g, TFLY_N_GAIT);
    float gate = TLearningGate(fly);
    if (gate <= 0.0f) return;
    float delta = 0.02f * fly->plasticity * gate * TClamp(score, -1.0f, 1.0f);
    int w = (int)fly->gait_weight[gi] + (int)(delta * 1000.0f);
    fly->gait_weight[gi] = (unsigned)TClamp((float)w, 0.0f, 1000.0f);
}

/* ------------------------------------------------------------------ *
 * Affect
 *
 * Affect is a state variable, not a mood label. These setters are the
 * intended way to call an emotion directly, as in:
 *     TFear(&fly, 0.8f);
 * ------------------------------------------------------------------ */

static inline void TEmotion(TFLY *fly, int emotion, float intensity) {
    if (fly == NULL) return;
    int e = TClampIdx(emotion, TFLY_N_EMOTION);
    fly->emotion[e] = TClamp(fly->emotion[e] + TClamp(intensity, 0.0f, 1.0f), 0.0f, 1.0f);
}

static inline void TFear(TFLY *fly, float v) {
    TEmotion(fly, T_EMO_FEAR, v);
    if (fly != NULL) {
        fly->emotion[T_EMO_JOY] *= (1.0f - 0.4f * TClamp(v, 0.0f, 1.0f));
        fly->emotion[T_EMO_CURIOSITY] *= (1.0f - 0.3f * TClamp(v, 0.0f, 1.0f));
        TEmit(fly, T_H_OCTOPAMINE, 0.3f * TClamp(v, 0.0f, 1.0f));
    }
}

static inline void TJoy(TFLY *fly, float v) {
    TEmotion(fly, T_EMO_JOY, v);
    if (fly != NULL) {
        fly->emotion[T_EMO_FEAR] *= (1.0f - 0.5f * TClamp(v, 0.0f, 1.0f));
        TEmit(fly, T_H_DOPAMINE, 0.35f * TClamp(v, 0.0f, 1.0f));
        TEmit(fly, T_H_NEUROPEPTIDE_F, 0.25f * TClamp(v, 0.0f, 1.0f));
    }
}

static inline void TSadness(TFLY *fly, float v) { TEmotion(fly, T_EMO_SADNESS, v); }
static inline void TAnger(TFLY *fly, float v) {
    TEmotion(fly, T_EMO_ANGER, v);
    if (fly != NULL) {
        fly->rival_pressure = TClamp(fly->rival_pressure + 0.2f * TClamp(v, 0.0f, 1.0f), 0.0f, 1.0f);
        TEmit(fly, T_H_OCTOPAMINE, 0.25f * TClamp(v, 0.0f, 1.0f));
    }
}
static inline void TDisgust(TFLY *fly, float v) { TEmotion(fly, T_EMO_DISGUST, v); }
static inline void TSurprise(TFLY *fly, float v) { TEmotion(fly, T_EMO_SURPRISE, v); }
static inline void TCuriosity(TFLY *fly, float v) { TEmotion(fly, T_EMO_CURIOSITY, v); }
static inline void TLust(TFLY *fly, float v) { TEmotion(fly, T_EMO_LUST, v); }
static inline void TCraving(TFLY *fly, float v) { TEmotion(fly, T_EMO_CRAVING, v); }
static inline void TPride(TFLY *fly, float v) { TEmotion(fly, T_EMO_PRIDE, v); }
static inline void TConfusion(TFLY *fly, float v) { TEmotion(fly, T_EMO_CONFUSION, v); }
static inline void TDread(TFLY *fly, float v) { TEmotion(fly, T_EMO_DREAD, v); }
static inline void TContentment(TFLY *fly, float v) { TEmotion(fly, T_EMO_CONTENTMENT, v); }

/* A negative emotion, e.g. a dampener for valence. */
static inline void TSadMood(TFLY *fly, float v) {
    if (fly == NULL) return;
    TSadness(fly, v);
    fly->emotion[T_EMO_CONTENTMENT] *= (1.0f - 0.5f * TClamp(v, 0.0f, 1.0f));
}

/* Recompute the two-axis affect summary from the discrete emotions. */
static inline void TRecomputeValence(TFLY *fly) {
    if (fly == NULL) return;
    float v = 0.0f, a = 0.0f;
    for (int i = 0; i < TFLY_N_EMOTION; i++) a += fly->emotion[i];
    v = (fly->emotion[T_EMO_JOY] + fly->emotion[T_EMO_CONTENTMENT] + fly->emotion[T_EMO_PRIDE] +
         fly->emotion[T_EMO_LUST]) -
        (fly->emotion[T_EMO_FEAR] + fly->emotion[T_EMO_PAIN] + fly->emotion[T_EMO_SADNESS] +
         fly->emotion[T_EMO_ANGER] + fly->emotion[T_EMO_DISGUST] + fly->emotion[T_EMO_DREAD]);
    fly->valence = TClamp(v * 0.5f, -1.0f, 1.0f);
    fly->arousal = TClamp(a / 6.0f + fly->emotion[T_EMO_FEAR] + fly->emotion[T_EMO_ANGER], 0.0f, 1.0f);
}

/* ------------------------------------------------------------------ *
 * Drives
 * ------------------------------------------------------------------ */

static inline void TDrive(TFLY *fly, int drive, float value) {
    if (fly == NULL) return;
    fly->drive[TClampIdx(drive, TFLY_N_DRIVE)] = TClamp(value, 0.0f, 1.0f);
}
static inline void THunger(TFLY *fly, float v) { TDrive(fly, T_DRIVE_HUNGER, v); }
static inline void TThirst(TFLY *fly, float v) { TDrive(fly, T_DRIVE_THIRST, v); }
static inline void TSexDrive(TFLY *fly, float v) {
    TDrive(fly, T_DRIVE_SEX, v);
    if (fly != NULL) fly->emotion[T_EMO_LUST] = TClamp(fly->emotion[T_EMO_LUST] + 0.3f * TClamp(v, 0.0f, 1.0f), 0.0f, 1.0f);
}
static inline void TSleepNeed(TFLY *fly, float v) { TDrive(fly, T_DRIVE_SLEEP, v); }
static inline void TSocial(TFLY *fly, float v) { TDrive(fly, T_DRIVE_SOCIAL, v); }
static inline void TCuriosityDrive(TFLY *fly, float v) { TDrive(fly, T_DRIVE_CURIOSITY, v); }

/* ------------------------------------------------------------------ *
 * Attention
 * ------------------------------------------------------------------ */

static inline void TAttention(TFLY *fly, int channel, float gain) {
    if (fly == NULL) return;
    fly->gain[TClampIdx(channel, TFLY_N_SENSORY)] = TClamp(gain, 0.0f, 2.0f);
    TRecompute(fly);
}

/* Shrink every channel's gain toward 1, i.e. attention fatigue. */
static inline void TAttentionFatigue(TFLY *fly, float rate) {
    if (fly == NULL) return;
    for (int i = 0; i < TFLY_N_SENSORY; i++) {
        fly->gain[i] = TLerp(fly->gain[i], 1.0f, TClamp(rate, 0.0f, 1.0f));
    }
}

/* ------------------------------------------------------------------ *
 * Working memory
 * ------------------------------------------------------------------ */

static inline void TMemorize(TFLY *fly, float key, float value) {
    if (fly == NULL) return;
    if (fly->memory_n < TFLY_N_MEMORY) {
        fly->memory[fly->memory_n][0] = key;
        fly->memory[fly->memory_n][1] = value;
        fly->memory_n++;
    } else {
        /* ring overwrite of the oldest entry */
        for (int i = 0; i < TFLY_N_MEMORY - 1; i++) {
            fly->memory[i][0] = fly->memory[i + 1][0];
            fly->memory[i][1] = fly->memory[i + 1][1];
        }
        fly->memory[TFLY_N_MEMORY - 1][0] = key;
        fly->memory[TFLY_N_MEMORY - 1][1] = value;
    }
}

static inline float TRecall(TFLY *fly, float key) {
    if (fly == NULL) return 0.0f;
    for (int i = 0; i < fly->memory_n; i++) {
        if (TAbs(fly->memory[i][0] - key) < 1e-3f) return fly->memory[i][1];
    }
    return 0.0f;
}

static inline void TForget(TFLY *fly) {
    if (fly == NULL) return;
    fly->memory_n = 0;
    memset(fly->memory, 0, sizeof(fly->memory));
}

/* ------------------------------------------------------------------ *
 * Learning
 *
 * TAssociate implements the three-factor rule used in insect conditioning:
 * a weight changes only when a neural trace of the action is present (factor
 * 1), an outcome is delivered (factor 2), and a modulator gates the update
 * (factor 3). Remove the modulator and no learning happens, which is exactly
 * why flies can learn a neutral colour but not learn while dopamine is low.
 * ------------------------------------------------------------------ */

static inline void TAssociate(TFLY *fly, int cue, int action, float outcome, float modulator) {
    if (fly == NULL) return;
    int c = TClampIdx(cue, TFLY_N_CUE);
    int a = TClampIdx(action, TFLY_N_ACTION);
    float m = TClamp(modulator, 0.0f, 2.0f);
    if (m <= 0.0f) return;

    float lr = 0.02f * fly->plasticity;
    float delta = lr * fly->elig[a] * TClamp(outcome, -1.0f, 1.0f) * m;
    fly->assoc[c][a] = TClamp(fly->assoc[c][a] + delta, -1.0f, 1.0f);
    fly->cue_value[c] = TClamp(fly->cue_value[c] + 0.05f * TClamp(outcome, -1.0f, 1.0f) * m, -1.0f, 1.0f);
}

/* Convenience: deliver an unconditioned stimulus and update every active cue. */
static inline void TTrial(TFLY *fly, float outcome, float modulator) {
    if (fly == NULL) return;
    for (int c = 0; c < TFLY_N_CUE; c++) {
        if (fly->sensory_raw[c] > 0.05f) TAssociate(fly, c, T_ACT_FORWARD, outcome, modulator);
    }
    for (int a = 0; a < TFLY_N_ACTION; a++) {
        if (fly->action[a] > 0.2f) TAssociate(fly, T_CUE_VISUAL, a, outcome * 0.2f, modulator);
    }
}

static inline void TReward(TFLY *fly, float amount) {
    if (fly == NULL) return;
    float v = TClamp(amount, 0.0f, 1.0f);
    TEmit(fly, T_H_NEUROPEPTIDE_F, v);
    TEmit(fly, T_H_DOPAMINE, v);
    fly->emotion[T_EMO_JOY] = TClamp(fly->emotion[T_EMO_JOY] + v, 0.0f, 1.0f);
    fly->emotion[T_EMO_PRIDE] = TClamp(fly->emotion[T_EMO_PRIDE] + 0.5f * v, 0.0f, 1.0f);
}

/*
 * Aversion. This releases octopamine, not serotonin.
 *
 * Serotonin would be the intuitive choice, but in this model serotonin is the
 * satiety signal and it suppresses the learning gate. Using it here creates a
 * doom loop: a fly that guesses wrong gets punished, the gate closes, and a
 * closed gate means it can never learn to stop guessing wrong. Punishment has
 * to open the gate for avoidance learning to be possible at all, which is also
 * what octopamine does in the real insect system.
 */
static inline void TPunish(TFLY *fly, float amount) {
    if (fly == NULL) return;
    float v = TClamp(amount, 0.0f, 1.0f);
    TEmit(fly, T_H_OCTOPAMINE, v);
    fly->emotion[T_EMO_SADNESS] = TClamp(fly->emotion[T_EMO_SADNESS] + v, 0.0f, 1.0f);
    fly->emotion[T_EMO_JOY] *= (1.0f - 0.6f * v);
    fly->stress = TClamp(fly->stress + 0.2f * v, 0.0f, 1.0f);
}

static inline void TPlasticity(TFLY *fly, float rate) {
    if (fly != NULL) fly->plasticity = TClamp(rate, 0.0f, 3.0f);
}

/* ------------------------------------------------------------------ *
 * Motor primitives. These set the action vector directly.
 * ------------------------------------------------------------------ */

static inline void TAct(TFLY *fly, int action, float amount) {
    if (fly == NULL) return;
    fly->action[TClampIdx(action, TFLY_N_ACTION)] = TClamp(amount, 0.0f, 1.0f);
}

static inline void TThrust(TFLY *fly, float v) {
    if (fly != NULL) fly->action[T_ACT_FORWARD] = TClamp(v, 0.0f, 1.0f);
}
static inline void TTurn(TFLY *fly, float v) {
    if (fly == NULL) return;
    fly->action[T_ACT_TURN_R] = TClamp(v, 0.0f, 1.0f);
    fly->action[T_ACT_TURN_L] = TClamp(1.0f - TClamp(v, 0.0f, 1.0f), 0.0f, 1.0f);
}
static inline void TLift(TFLY *fly, float v) {
    if (fly == NULL) return;
    fly->action[T_ACT_UP] = TClamp(v, 0.0f, 1.0f);
    fly->action[T_ACT_DOWN] = TClamp(1.0f - TClamp(v, 0.0f, 1.0f), 0.0f, 1.0f);
}
static inline void TWingbeat(TFLY *fly, float hz) {
    if (fly != NULL) fly->action[T_ACT_FLAP] = TClamp(hz / 200.0f, 0.0f, 1.0f);
}
static inline void TDance(TFLY *fly, float intensity) { TAct(fly, T_ACT_DANCE, intensity); }
static inline void TEat(TFLY *fly, float v) { TAct(fly, T_ACT_EAT, v); }
static inline void TMate(TFLY *fly, float v) { TAct(fly, T_ACT_MATE, v); }
static inline void TRest(TFLY *fly, float v) { TAct(fly, T_ACT_REST, v); }

/* ------------------------------------------------------------------ *
 * Social signals
 * ------------------------------------------------------------------ */

static inline void TMateSignal(TFLY *fly, float quality) {
    if (fly == NULL) return;
    fly->mate_quality = TClamp(quality, 0.0f, 1.0f);
    fly->emotion[T_EMO_LUST] = TClamp(fly->emotion[T_EMO_LUST] + 0.4f * fly->mate_quality, 0.0f, 1.0f);
    fly->drive[T_DRIVE_SEX] = TClamp(fly->drive[T_DRIVE_SEX] + 0.3f * fly->mate_quality, 0.0f, 1.0f);
}

static inline void TRivalSignal(TFLY *fly, float pressure) {
    if (fly == NULL) return;
    fly->rival_pressure = TClamp(pressure, 0.0f, 1.0f);
    fly->emotion[T_EMO_ANGER] = TClamp(fly->emotion[T_EMO_ANGER] + 0.5f * fly->rival_pressure, 0.0f, 1.0f);
}

static inline void TPredatorSignal(TFLY *fly, float risk) {
    if (fly == NULL) return;
    fly->predator_risk = TClamp(risk, 0.0f, 1.0f);
    fly->emotion[T_EMO_FEAR] = TClamp(fly->emotion[T_EMO_FEAR] + 0.5f * fly->predator_risk, 0.0f, 1.0f);
    fly->emotion[T_EMO_DREAD] = TClamp(fly->emotion[T_EMO_DREAD] + 0.3f * fly->predator_risk, 0.0f, 1.0f);
    TEmit(fly, T_H_OCTOPAMINE, 0.4f * fly->predator_risk);
}

static inline void TTemperatureAmbient(TFLY *fly, float celsius) {
    if (fly == NULL) return;
    float norm = TClamp((celsius + 5.0f) / 40.0f, 0.0f, 1.0f);
    TTemperature(fly, norm);
    fly->drive[T_DRIVE_COLD] = TClamp(TAbs(norm - 0.5f) * 1.5f, 0.0f, 1.0f);
}

/* ------------------------------------------------------------------ *
 * Policy: emotions, drives and memory into motor output
 *
 * This is the only place where affect turns into behaviour. It is a linear
 * policy plus exploration noise, which keeps the whole loop inspectable.
 * ------------------------------------------------------------------ */

static inline void TPolicy(TFLY *fly, float dt) {
    if (fly == NULL) return;

    float fear = fly->emotion[T_EMO_FEAR] + fly->emotion[T_EMO_DREAD];
    float pain = fly->emotion[T_EMO_PAIN];
    float joy = fly->emotion[T_EMO_JOY] + fly->emotion[T_EMO_CONTENTMENT];
    float curiosity = fly->emotion[T_EMO_CURIOSITY];
    float anger = fly->emotion[T_EMO_ANGER];
    float sadness = fly->emotion[T_EMO_SADNESS];
    /* A hopeful fly walks faster and straighter; a bored one dawdles. These
     * feed the gait, which is the part the fly is actually learning. */
    float hope = fly->emotion[T_EMO_HOPE];
    float boredom = fly->emotion[T_EMO_BOREDOM];
    float excitement = fly->emotion[T_EMO_EXCITEMENT] + fly->emotion[T_EMO_JOY];
    float affection = fly->emotion[T_EMO_AFFECTION];

    /* exploration scales with curiosity, and is suppressed by fear */
    float explore = 0.05f + 0.35f * curiosity * (1.0f - 0.7f * fear);

    float forward = (1.0f - fly->drive[T_DRIVE_HUNGER]) * 0.3f +
                    joy * 0.4f + curiosity * 0.2f - fear * 0.3f + TGauss(fly) * explore;
    float rest = fly->drive[T_DRIVE_SLEEP] * 0.8f + fly->drive[T_DRIVE_FATIGUE] * 0.5f +
                 sadness * 0.3f;
    float turn = fly->odor_lateral * 0.6f + fly->target_x * 0.5f + TGauss(fly) * explore * 1.5f +
                 anger * 0.4f;
    float lift = (0.5f - fly->sensory[T_TEMPERATURE]) * 0.3f + fear * 0.6f - curiosity * 0.2f;
    float flap = 0.15f + fly->arousal * 0.5f + fear * 0.4f + joy * 0.2f;
    float dance = joy * 0.6f + fly->drive[T_DRIVE_SEX] * fly->mate_quality * 0.8f;
    float mate = fly->drive[T_DRIVE_SEX] * fly->mate_quality;
    float eat = (1.0f - fly->drive[T_DRIVE_HUNGER]) * fly->sensory[T_TASTE];

    for (int i = 0; i < TFLY_N_ACTION; i++) fly->action[i] *= 0.55f; /* action persistence */
    fly->action[T_ACT_REST] = TClamp(fly->action[T_ACT_REST] + TClamp(rest, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);
    fly->action[T_ACT_FORWARD] = TClamp(fly->action[T_ACT_FORWARD] + TClamp(forward, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);
    fly->action[T_ACT_TURN_L] = TClamp(fly->action[T_ACT_TURN_L] + TClamp(-turn, 0.0f, 1.0f) * 0.35f, 0.0f, 1.0f);
    fly->action[T_ACT_TURN_R] = TClamp(fly->action[T_ACT_TURN_R] + TClamp(turn, 0.0f, 1.0f) * 0.35f, 0.0f, 1.0f);
    fly->action[T_ACT_UP] = TClamp(fly->action[T_ACT_UP] + TClamp(lift, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);
    fly->action[T_ACT_DOWN] = TClamp(fly->action[T_ACT_DOWN] + TClamp(-lift, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);
    fly->action[T_ACT_FLAP] = TClamp(fly->action[T_ACT_FLAP] + TClamp(flap, 0.0f, 1.0f) * 0.4f, 0.0f, 1.0f);
    fly->action[T_ACT_DANCE] = TClamp(fly->action[T_ACT_DANCE] + TClamp(dance, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);
    fly->action[T_ACT_EAT] = TClamp(fly->action[T_ACT_EAT] + TClamp(eat, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);
    fly->action[T_ACT_MATE] = TClamp(fly->action[T_ACT_MATE] + TClamp(mate, 0.0f, 1.0f) * 0.3f, 0.0f, 1.0f);

    /* fear must not be talked out of by a positive drive */
    if (fear > 0.5f) {
        fly->action[T_ACT_EAT] *= (1.0f - fear);
        fly->action[T_ACT_MATE] *= (1.0f - fear);
        fly->action[T_ACT_DANCE] *= (1.0f - fear);
    }

    /* rest suppresses flight, otherwise a tired fly flaps itself to exhaustion
     * and the fatigue drive becomes unsatisfiable */
    float rest_gate = TClamp(rest, 0.0f, 1.0f);
    fly->action[T_ACT_FLAP] *= (1.0f - 0.85f * rest_gate);
    fly->action[T_ACT_FORWARD] *= (1.0f - 0.5f * rest_gate);
    fly->action[T_ACT_DANCE] *= (1.0f - rest_gate);

    /* motor outputs */
    fly->out_thrust = TClamp(fly->action[T_ACT_FORWARD] * 0.6f + fly->action[T_ACT_FLAP] * 0.3f, 0.0f, 1.0f);
    fly->out_turn = TClamp(fly->action[T_ACT_TURN_R] - fly->action[T_ACT_TURN_L], -1.0f, 1.0f);
    fly->out_vertical = TClamp(fly->action[T_ACT_UP] - fly->action[T_ACT_DOWN], -1.0f, 1.0f);
    float beat = 60.0f + 140.0f * fly->action[T_ACT_FLAP] + 60.0f * fear + 40.0f * joy;
    fly->out_wingbeat = beat * (1.0f - 0.9f * rest_gate);

    /* eligibility traces for the three-factor rule */
    float decay = expf(-dt / 0.8f);
    for (int a = 0; a < TFLY_N_ACTION; a++) {
        fly->elig[a] *= decay;
        fly->elig[a] += fly->action[a] * dt;
    }

    /* ---- gait targets ----
     * The fly does not choose a gait here; it chooses a *desire* for one, and
     * the walk itself is learned separately. Pain and fear clamp the stride
     * down, hope and excitement raise the cadence, boredom slows everything,
     * and affection toward a nearby mate keeps the fly near her. */
    float stride_want = 0.55f + 0.25f * hope + 0.20f * excitement - 0.45f * pain - 0.25f * fear;
    float cadence_want = 0.55f + 0.20f * excitement + 0.15f * curiosity - 0.30f * boredom +
                         0.20f * anger;
    float approach = 0.25f * fly->mate_quality * affection;

    fly->gait_stride = TExp(fly->gait_stride, TClamp(stride_want, 0.05f, 1.0f), 1.5f, dt);
    fly->gait_cadence = TExp(fly->gait_cadence, TClamp(cadence_want, 0.05f, 1.0f), 1.5f, dt);
    fly->gait_sway = TExp(fly->gait_sway, TClamp(0.3f + 0.4f * boredom - 0.2f * pain, 0.0f, 1.0f), 2.0f, dt);
    fly->gait_approach = TExp(fly->gait_approach, TClamp(approach, 0.0f, 1.0f), 3.0f, dt);
    (void)pain;
}

/* ------------------------------------------------------------------ *
 * TUpdate: one integration step
 * ------------------------------------------------------------------ */

static inline void TUpdate(TFLY *fly, float dt) {
    if (fly == NULL || dt <= 0.0f) return;
    if (dt > 0.25f) dt = 0.25f; /* never let a single step explode */

    fly->t += dt;
    fly->ticks++;
    fly->age += dt;

    /* ---- sensory gating, adaptation and novelty ---- */
    for (int i = 0; i < TFLY_N_SENSORY; i++) {
        float raw = fly->sensory_raw[i];
        float gated = raw * fly->gain[i];
        float delta = TAbs(gated - fly->adapt[i]);
        fly->novel[i] = TExp(fly->novel[i], TClamp(delta * 3.0f, 0.0f, 1.0f), 0.5f, dt);
        fly->adapt[i] = TExp(fly->adapt[i], gated * 0.6f, 8.0f, dt);
        fly->sensory[i] = TExp(fly->sensory[i], gated, 0.05f, dt);
        if (i == T_DARK || i == T_SMELL_ANTENNA) fly->sensory[i] = raw;
    }
    TRecompute(fly);

    /* ---- drives: accumulate, never spontaneously resolve ---- */
    for (int d = 0; d < TFLY_N_DRIVE; d++) {
        float rate = 0.004f + 0.02f * fly->drive[d];
        fly->drive[d] = TClamp(fly->drive[d] + rate * dt, 0.0f, 1.0f);
    }
    fly->drive[T_DRIVE_FATIGUE] = TClamp(1.0f - fly->energy, 0.0f, 1.0f);
    fly->drive[T_DRIVE_CURIOSITY] = TClamp(
        fly->drive[T_DRIVE_CURIOSITY] * 0.995f + 0.05f * fly->emotion[T_EMO_CURIOSITY], 0.0f, 1.0f);

    /* ---- pain clearance and emotional decay ---- */
    for (int r = 0; r < TFLY_N_REGION; r++) fly->pain[r] = TExp(fly->pain[r], 0.0f, 3.0f, dt);
    fly->pain_total = 0.0f;
    for (int r = 0; r < TFLY_N_REGION; r++) fly->pain_total += fly->pain[r];
    fly->pain_total = TClamp(fly->pain_total * 0.25f, 0.0f, 1.5f);
    fly->pain_memory = TExp(fly->pain_memory, 0.0f, 20.0f, dt);

    for (int e = 0; e < TFLY_N_EMOTION; e++) {
        fly->emotion[e] = TExp(fly->emotion[e], 0.0f, TFlyEmotionHalfLife(e), dt);
    }

    /* ---- hormone clearance toward the tonic set-point ---- */
    for (int h = 0; h < TFLY_N_HORMONE; h++) {
        float tau = 4.0f;
        if (h == T_H_ECDYSONE || h == T_H_JUVENILE_HORMONE) tau = 600.0f;
        else if (h == T_H_GLUTAMATE || h == T_H_GABA) tau = 0.2f;
        else if (h == T_H_INSULIN || h == T_H_LEPTIN) tau = 300.0f;
        fly->hormone[h] = TExp(fly->hormone[h], fly->hormone_set[h], tau, dt);
        fly->pulse[h] *= expf(-dt / 0.5f);
    }

    /* ---- neuromodulatory couplings, the interesting part ---- */
    float dop = fly->hormone[T_H_DOPAMINE];
    float oct = fly->hormone[T_H_OCTOPAMINE];
    float ser = fly->hormone[T_H_SEROTONIN];
    float npf = fly->hormone[T_H_NEUROPEPTIDE_F];
    float glu = fly->hormone[T_H_GLUTAMATE];
    float gaba = fly->hormone[T_H_GABA];
    float fear = TClamp(fly->emotion[T_EMO_FEAR] + fly->emotion[T_EMO_DREAD], 0.0f, 1.0f);

    fly->emotion[T_EMO_JOY] = TClamp(fly->emotion[T_EMO_JOY] + (dop * 0.02f + npf * 0.015f) * dt, 0.0f, 1.0f);
    fly->emotion[T_EMO_CONTENTMENT] = TClamp(fly->emotion[T_EMO_CONTENTMENT] + ser * 0.01f * dt, 0.0f, 1.0f);
    fly->emotion[T_EMO_ANGER] = TClamp(fly->emotion[T_EMO_ANGER] + oct * 0.01f * dt, 0.0f, 1.0f);
    fly->emotion[T_EMO_SURPRISE] = TClamp(fly->emotion[T_EMO_SURPRISE] + oct * 0.015f * dt, 0.0f, 1.0f);
    fly->emotion[T_EMO_CURIOSITY] = TClamp(fly->emotion[T_EMO_CURIOSITY] + fly->hormone[T_H_NOVELTY] * 0.02f * dt, 0.0f, 1.0f);
    fly->emotion[T_EMO_LUST] = TClamp(fly->emotion[T_EMO_LUST] + fly->hormone[T_H_JUVENILE_HORMONE] * 0.01f * dt, 0.0f, 1.0f);
    fly->emotion[T_EMO_ANGER] = TClamp(fly->emotion[T_EMO_ANGER] + fly->hormone[T_H_ECDYSONE] * 0.005f * dt, 0.0f, 1.0f);

    /* ---- the richer affect layer ----
     * These are derived, not stored ad hoc: relief falls as pain clears,
     * boredom grows when nothing new happens, excitement spikes on a reward
     * surge, and the social states decay very slowly because a companion is
     * remembered long after the moment has passed. */
    {
        float pain_now = 0.0f;
        for (int r = 0; r < TFLY_N_REGION; r++) pain_now += fly->pain[r];
        pain_now = TClamp(pain_now, 0.0f, 1.0f);

        /* Relief: the falling edge of pain, not the absence of it. */
        float relief = TClamp((fly->pain_memory - pain_now) * 0.5f, 0.0f, 1.0f);
        fly->emotion[T_EMO_RELIEF] =
            TExp(fly->emotion[T_EMO_RELIEF], relief, 2.0f, dt);

        /* Boredom: no novelty, no demand, nothing happening. */
        float novelty_now = 0.0f;
        for (int i = 0; i < TFLY_N_SENSORY; i++) novelty_now += fly->novel[i];
        float busy = TClamp(0.25f * pain_now + 0.2f * (1.0f - fly->energy) +
                                 0.3f * TClamp(novelty_now, 0.0f, 1.0f) +
                                 0.3f * fly->drive[T_DRIVE_SEX],
                             0.0f, 1.0f);
        fly->emotion[T_EMO_BOREDOM] = TExp(fly->emotion[T_EMO_BOREDOM], 1.0f - busy, 6.0f, dt);

        /* Excitement and hope ride the reward channel; disappointment is what
         * is left when a demand goes unmet while the fly is trying to meet it. */
        fly->emotion[T_EMO_EXCITEMENT] = TExp(fly->emotion[T_EMO_EXCITEMENT],
                                              TClamp(dop + npf * 0.5f, 0.0f, 1.0f), 2.0f, dt);
        float unmet = TClamp(fly->drive[T_DRIVE_HUNGER] - (1.0f - fly->energy), 0.0f, 1.0f);
        fly->emotion[T_EMO_HOPE] = TExp(fly->emotion[T_EMO_HOPE],
                                        TClamp(0.5f + 0.5f * fly->energy - unmet, 0.0f, 1.0f),
                                        8.0f, dt);
        fly->emotion[T_EMO_DISAPPOINTMENT] =
            TExp(fly->emotion[T_EMO_DISAPPOINTMENT], TClamp(unmet - 0.2f, 0.0f, 1.0f), 5.0f, dt);

        /* Social states are slow. They are driven by whether anyone is around
         * at all, which here means a mate or a rival signal being present. */
        float company = TClamp(fly->mate_quality, 0.0f, 1.0f);
        float threat = TClamp(fly->rival_pressure + fly->predator_risk, 0.0f, 1.0f);
        fly->emotion[T_EMO_AFFECTION] = TExp(fly->emotion[T_EMO_AFFECTION], company, 12.0f, dt);
        fly->emotion[T_EMO_TRUST] = TExp(fly->emotion[T_EMO_TRUST], company * (1.0f - threat), 20.0f, dt);
        fly->emotion[T_EMO_LONGING] = TExp(fly->emotion[T_EMO_LONGING],
                                           TClamp(fly->drive[T_DRIVE_SEX] * (1.0f - company), 0.0f, 1.0f),
                                           15.0f, dt);
        fly->emotion[T_EMO_JEALOUSY] = TExp(fly->emotion[T_EMO_JEALOUSY],
                                            TClamp(company * threat, 0.0f, 1.0f), 12.0f, dt);
        fly->emotion[T_EMO_SHYNESS] = TExp(fly->emotion[T_EMO_SHYNESS],
                                           TClamp(company * (1.0f - fly->energy) * 0.8f, 0.0f, 1.0f),
                                           8.0f, dt);
        fly->emotion[T_EMO_COMPASSION] = TExp(fly->emotion[T_EMO_COMPASSION],
                                              TClamp(fly->emotion[T_EMO_SADNESS] * 0.5f, 0.0f, 1.0f),
                                              10.0f, dt);
        fly->emotion[T_EMO_GRATITUDE] = TExp(fly->emotion[T_EMO_GRATITUDE],
                                             TClamp(company * 0.3f + (1.0f - pain_now) * 0.2f, 0.0f, 1.0f),
                                             9.0f, dt);
    }

    /* excitation and inhibition shift sensory gain and the stress ceiling */
    for (int i = 0; i < TFLY_N_SENSORY; i++) {
        fly->gain[i] = TClamp(fly->gain[i] + (glu - gaba) * 0.01f * dt, 0.0f, 2.0f);
    }
    float stress_cap = 1.0f - 0.4f * fly->hormone[T_H_SEROTONIN];
    fly->stress = TClamp(fly->stress, 0.0f, TClamp(stress_cap, 0.2f, 1.0f));

    /* State-dependent learning gate. This is the single most important knob
     * for making a fly feel like it has moods rather than moods bolted on:
     * the same event is learned differently depending on the internal state.
     *   - dopamine  : appetitive learning, the "this was good" signal
     *   - octopamine: aversive learning and arousal, the "this mattered" signal
     *   - serotonin : satiety and behavioural switching, it *closes* the gate
     *   - darkness  : octopamine dominates, so the night fly learns harder
     */
    float dark = fly->sensory[T_DARK];
    float gate = 0.15f + 0.55f * dop + 0.45f * oct + 0.25f * npf - 0.35f * ser + 0.20f * dark;
    gate = TClamp(gate, 0.0f, 1.0f);
    TPlasticity(fly, TExp(fly->plasticity, gate, 20.0f, dt));

    /* A surprise pulse: when the sensory scene changes faster than the
     * adaptation baseline can follow, the fly is cued to explore, and the
     * novelty hormone is released. This is what makes curiosity self
     * starting instead of something the caller has to switch on. */
    float surprise_rate = 0.0f;
    for (int i = 0; i < TFLY_N_SENSORY; i++) surprise_rate += fly->novel[i];
    surprise_rate *= 0.06f;
    TEmit(fly, T_H_NOVELTY, surprise_rate * dt);
    float cur = fly->emotion[T_EMO_CURIOSITY];
    float cur_target = TClamp(surprise_rate * 0.5f + 0.15f * fly->drive[T_DRIVE_CURIOSITY] -
                                  0.5f * fear,
                              0.0f, 1.0f);
    fly->emotion[T_EMO_CURIOSITY] = TExp(cur, cur_target, 1.0f, dt);
    fly->emotion[T_EMO_SURPRISE] =
        TExp(fly->emotion[T_EMO_SURPRISE], TClamp(surprise_rate, 0.0f, 1.0f), 1.5f, dt);

    /* ---- behaviour ---- */
    if (fly->asleep) {
        /* Motor output must actually stop while asleep, not merely stop being
         * updated. A sleeping fly is quiescent: wings folded, no thrust. */
        for (int a = 0; a < TFLY_N_ACTION; a++) fly->action[a] *= expf(-dt / 0.2f);
        fly->action[T_ACT_REST] = 1.0f;
        fly->out_thrust = 0.0f;
        fly->out_turn = 0.0f;
        fly->out_vertical = 0.0f;
        fly->out_wingbeat = 0.0f;
        fly->balance = TExp(fly->balance, 1.0f, 2.0f, dt);
    } else {
        TPolicy(fly, dt);
    }

    /* ---- walking ----
     * These flies do not fly. The gait is integrated on the ground plane, and
     * the "wingbeat" output now drives the walk cycle, which is why it is
     * called that: it is the limb oscillator, not a wing. Sway and a mismatch
     * between the learned stride and the desired one cost balance, and a fly
     * that loses its balance is punished for it, which is what gives the
     * learning something to converge on. */
    {
        float pace = TClamp(fly->out_thrust, 0.0f, 1.0f);
        float hz = 0.6f + 3.4f * fly->gait_cadence;     /* steps per second */
        float step_len = 0.10f + 0.22f * fly->gait_stride;
        float prev_phase = fly->gait_phase;
        fly->gait_phase += hz * dt * 6.2831853f;
        /* Count whole steps, not phase revolutions. */
        float before = (float)(long)(prev_phase / 3.14159265f);
        float after = (float)(long)(fly->gait_phase / 3.14159265f);
        if (after > before) {
            int steps = (int)(after - before);
            fly->steps += (float)steps;
            /* Landing a step costs energy in proportion to how long it was. */
            fly->energy = TClamp(fly->energy - 0.0009f * step_len * (float)steps, 0.0f, 1.0f);
        }
        /* A large sway while moving fast is a fall waiting to happen. */
        float instability = fly->gait_sway * pace * (0.4f + 0.6f * fly->gait_stride);
        fly->balance = TClamp(fly->balance - 0.25f * instability * dt, 0.0f, 1.0f);
        if (fly->balance < 0.25f) {
            /* Stumble: slow down and bleed balance back. */
            fly->out_thrust *= 0.4f;
            fly->balance = TClamp(fly->balance + 0.25f * dt, 0.0f, 1.0f);
        }
        fly->stress = TClamp(fly->stress + 0.02f * (1.0f - fly->balance) * dt, 0.0f, 1.0f);
        /* Walking burns energy through the legs, not through flight. */
        fly->energy = TClamp(fly->energy - 0.0022f * pace * dt *
                                             (1.0f - 0.3f * fly->hormone[T_H_INSULIN]),
                             0.0f, 1.0f);
    }

    /* ---- physiology ----
     * Resting must actually pay off. A fly that stops stepping both
     * burns far less and recovers, so the rest term is gated on the action
     * vector rather than on a fixed bonus. */
    float resting = fly->asleep ? 1.0f : TClamp(fly->action[T_ACT_REST], 0.0f, 1.0f);
    float burn = 0.0004f + 0.0016f * fly->out_thrust + 0.0010f * fly->action[T_ACT_DANCE];
    burn *= (1.0f - 0.9f * resting);
    fly->energy = TClamp(fly->energy - burn * dt * (1.0f - 0.3f * fly->hormone[T_H_INSULIN]), 0.0f, 1.0f);
    if (fly->action[T_ACT_EAT] > 0.3f) fly->energy = TClamp(fly->energy + 0.01f * dt, 0.0f, 1.0f);
    fly->energy = TClamp(fly->energy + 0.006f * resting * dt, 0.0f, 1.0f);
    fly->stress = TClamp(fly->stress - 0.01f * dt + fly->emotion[T_EMO_DREAD] * 0.01f * dt, 0.0f, 1.0f);
    fly->temp_body = TExp(fly->temp_body, 0.5f + 0.2f * fly->out_thrust, 10.0f, dt);
    fly->lifespan = TClamp(1.0f - fly->age / 86400.0f, 0.0f, 1.0f);

    /* ---- encounter cooldown ----
     * This has to be discharged here, or a fly that has ever met another one
     * can never meet anyone again and the social layer silently switches off
     * after the first greeting. */
    fly->encounter_cool = TMax(0.0f, fly->encounter_cool - dt);

    /* ---- the face ----
     * Every channel here is derived from the fly's own state, so the rig never
     * has to invent an expression. Eyes first, because a character looks at
     * you before it reacts to you.
     *
     * Blinking is not a timer. Drowsiness closes the lids on its own, pain
     * forces them shut, and surprise suppresses the blink entirely. */
    {
        /* Re-derive the handful of affect scalars this block needs. They are
         * cheap to recompute and it keeps the face block self-contained. */
        float face_pain = 0.0f;
        for (int r = 0; r < TFLY_N_REGION; r++) face_pain += fly->pain[r];
        face_pain = TClamp(face_pain, 0.0f, 1.0f);
        float face_fear = TClamp(fly->emotion[T_EMO_FEAR] + fly->emotion[T_EMO_DREAD], 0.0f, 1.0f);
        float face_joy = fly->emotion[T_EMO_JOY] + fly->emotion[T_EMO_CONTENTMENT];
        float face_anger = fly->emotion[T_EMO_ANGER];
        float drowsy = 1.0f - fly->energy;
        float want_rate = 0.35f + 0.5f * drowsy - 0.25f * (face_fear + fly->emotion[T_EMO_SURPRISE]);
        fly->blink_rate = TClamp(want_rate, 0.05f, 1.5f);
        if (fly->asleep) {
            fly->blink = TExp(fly->blink, 1.0f, 0.1f, dt);
        } else {
            float period = 2.6f / fly->blink_rate;
            float in_cycle = fmodf(fly->age, period) / period;
            float closed = (in_cycle < 0.15f) ? 1.0f : 0.0f;
            /* Surprise holds the lids open rather than letting them blink. */
            if (fly->emotion[T_EMO_SURPRISE] > 0.6f) closed = 0.0f;
            fly->blink = TExp(fly->blink, closed, 0.05f, dt);
        }
        if (face_pain > 0.5f) fly->blink = TClamp(fly->blink + 0.4f, 0.0f, 1.0f);

        /* Gaze: a mate, a rival, a threat, or a drift when nothing matters. */
        float gaze_want_x = 0.0f;
        float gaze_want_y = 0.0f;
        if (fly->mate_quality > 0.05f) gaze_want_x = fly->mate_quality * 0.5f;
        if (fly->rival_pressure > 0.05f) gaze_want_x += fly->rival_pressure * 0.4f;
        if (fly->predator_risk > 0.05f) {
            gaze_want_x -= fly->predator_risk * 0.4f;
            gaze_want_y += fly->predator_risk * 0.5f;
        }
        if (fabsf(gaze_want_x) < 0.05f && fabsf(gaze_want_y) < 0.05f) {
            gaze_want_x = 0.25f * sinf(fly->age * 0.7f);
            gaze_want_y = 0.15f * sinf(fly->age * 0.43f + 1.0f);
        }
        /* A closing lid drags the gaze down with it. */
        gaze_want_y -= fly->blink * 0.3f;
        fly->gaze_x = TExp(fly->gaze_x, TClamp(gaze_want_x, -1.0f, 1.0f), 0.35f, dt);
        fly->gaze_y = TExp(fly->gaze_y, TClamp(gaze_want_y, -1.0f, 1.0f), 0.35f, dt);

        /* Pupils: face_fear dilates, contentment and sleep constrict. */
        float pupil_want = 0.75f + 0.55f * (face_fear + fly->emotion[T_EMO_SURPRISE]) -
                           0.30f * (fly->emotion[T_EMO_CONTENTMENT] + 0.5f * drowsy);
        fly->pupil = TExp(fly->pupil, TClamp(pupil_want, 0.5f, 1.6f), 0.4f, dt);

        /* Brows: up in alarm, down in face_anger and grief. */
        float brow_want = 0.6f * fly->emotion[T_EMO_SURPRISE] + 0.5f * face_fear -
                          0.6f * face_anger - 0.4f * fly->emotion[T_EMO_SADNESS] -
                          0.3f * fly->emotion[T_EMO_CONFUSION];
        fly->brow = TExp(fly->brow, TClamp(brow_want, -1.0f, 1.0f), 0.25f, dt);

        /* The smile is the clearest single readout of mood. */
        float smile = 0.7f * (face_joy + fly->emotion[T_EMO_AFFECTION] + fly->emotion[T_EMO_HOPE]) -
                      0.8f * (face_fear + fly->emotion[T_EMO_PAIN] + fly->emotion[T_EMO_DISAPPOINTMENT]) -
                      0.4f * face_anger;
        fly->mouth = TExp(fly->mouth, TClamp(smile, -1.0f, 1.0f), 0.3f, dt);
        float open_want = 0.7f * fly->emotion[T_EMO_SURPRISE] + 0.4f * drowsy +
                          0.5f * fly->emotion[T_EMO_EXCITEMENT] + 0.6f * face_pain;
        fly->mouth_open = TExp(fly->mouth_open, TClamp(open_want, 0.0f, 1.0f), 0.2f, dt);

        float blush_want = 0.8f * (fly->emotion[T_EMO_SHYNESS] + fly->emotion[T_EMO_AFFECTION] +
                                    fly->emotion[T_EMO_EXCITEMENT]);
        fly->blush = TExp(fly->blush, TClamp(blush_want, 0.0f, 1.0f), 2.0f, dt);
        /* Tears are a wetness reservoir, not a mirror of sadness. Sadness
         * comes and goes quickly; tears lag behind it and then dry slowly, so
         * a character can still be crying after she has stopped being sad. The
         * gain has to clear the drying term, or a fly who is briefly sad never
         * actually sheds a tear. */
        float tear_gain = fly->emotion[T_EMO_SADNESS] * 0.35f;
        float tear_dry =
            0.03f + 0.22f * face_joy + 0.10f * fly->emotion[T_EMO_CONTENTMENT];
        /* The ceiling is below 1.0 on purpose: a face streaming at maximum
         * while smiling reads as a rendering fault rather than as sadness. */
        float tears = fly->tears + (tear_gain - tear_dry) * dt;
        fly->tears = TClamp(tears, 0.0f, 0.85f);
        /* Sweat tracks nerves: stress plus awkwardness. */
        float sweat_rate = (fly->stress * 0.4f + fly->emotion[T_EMO_SHYNESS] * 0.5f) - 0.03f;
        fly->sweat = TClamp(fly->sweat + sweat_rate * dt, 0.0f, 1.0f);
    }

    /* ---- gesture envelope ----
     * A gesture runs for a fixed time with a soft start and end, so the rig
     * can pose from `gesture_phase` without easing the strength itself. */
    if (fly->gesture != T_GES_IDLE) {
        fly->gesture_phase += dt / GESTURE_SECONDS;
        if (fly->gesture_phase >= 1.0f) {
            fly->gesture_phase = 0.0f;
            fly->gesture = T_GES_IDLE;
            fly->gesture_strength = 0.0f;
        } else {
            fly->gesture_strength = 0.5f - 0.5f * cosf(6.2831853f * fly->gesture_phase);
        }
    } else {
        fly->gesture_strength = TExp(fly->gesture_strength, 0.0f, 0.1f, dt);
    }

    /* ---- sleep gating ----
     * Sleep pressure has to fall while the fly sleeps, otherwise the state
     * is absorbing: once asleep the fly can never build up the reason to
     * wake up, and the run deadlocks with a full energy store. Sleeping is
     * what discharges the drive, and waking restores it slowly. */
    if (fly->asleep) {
        fly->drive[T_DRIVE_SLEEP] = TClamp(fly->drive[T_DRIVE_SLEEP] - 0.05f * dt, 0.0f, 1.0f);
        fly->drive[T_DRIVE_CURIOSITY] = TClamp(fly->drive[T_DRIVE_CURIOSITY] - 0.02f * dt, 0.0f, 1.0f);
        /* sleep also clears stress, which is the other thing that has to be
         * discharged for the fly to be able to face the world again */
        fly->stress = TClamp(fly->stress - 0.03f * dt, 0.0f, 1.0f);
    }
    if (fly->drive[T_DRIVE_SLEEP] > 0.9f || fly->energy < 0.05f) fly->asleep = 1;
    if (fly->asleep && fly->drive[T_DRIVE_SLEEP] < 0.2f) fly->asleep = 0;

    TRecomputeValence(fly);

    /* transient signals decay back to zero */
    fly->odor_lateral *= expf(-dt / 2.0f);
    fly->rival_pressure *= expf(-dt / 5.0f);
    fly->predator_risk *= expf(-dt / 5.0f);
    TAttentionFatigue(fly, 0.01f * dt);
}

static inline void TTick(TFLY *fly, float dt) {
    TUpdate(fly, dt);
}

static inline void TSteps(TFLY *fly, int n, float dt) {
    for (int i = 0; i < n; i++) TUpdate(fly, dt);
}

static inline void TSleep(TFLY *fly) {
    if (fly == NULL) return;
    fly->asleep = 1;
    fly->drive[T_DRIVE_SLEEP] = 1.0f;
}
static inline void TWake(TFLY *fly) {
    if (fly == NULL) return;
    fly->asleep = 0;
    fly->drive[T_DRIVE_SLEEP] = 0.0f;
}

/* ------------------------------------------------------------------ *
 * Introspection
 * ------------------------------------------------------------------ */

static inline float TEmotionLevel(TFLY *fly, int emotion) {
    if (fly == NULL) return 0.0f;
    return fly->emotion[TClampIdx(emotion, TFLY_N_EMOTION)];
}
static inline float TDriveLevel(TFLY *fly, int drive) {
    if (fly == NULL) return 0.0f;
    return fly->drive[TClampIdx(drive, TFLY_N_DRIVE)];
}
static inline float THormoneLevel(TFLY *fly, int hormone) {
    if (fly == NULL) return 0.0f;
    return fly->hormone[TClampIdx(hormone, TFLY_N_HORMONE)];
}
static inline float TSensoryLevel(TFLY *fly, int channel) {
    if (fly == NULL) return 0.0f;
    return fly->sensory[TClampIdx(channel, TFLY_N_SENSORY)];
}
static inline float TActionLevel(TFLY *fly, int action) {
    if (fly == NULL) return 0.0f;
    return fly->action[TClampIdx(action, TFLY_N_ACTION)];
}

/* Dominant emotion, for a single-word "mood" readout. */
/*
 * Emotional decay is not uniform. Nociception and surprise are fast and
 * short-lived, contentment and dread are slow and persistent. A fly that
 * forgets a shock in half a second and a fly that holds a grudge for a minute
 * are very different animals, and this is the knob that tells them apart.
 */
static inline float TFlyEmotionHalfLife(int e) {
    switch (TClampIdx(e, TFLY_N_EMOTION)) {
        case T_EMO_PAIN: return 2.0f;
        case T_EMO_SURPRISE: return 1.5f;
        case T_EMO_CONFUSION: return 2.0f;
        case T_EMO_ANGER: return 3.0f;
        case T_EMO_DISGUST: return 4.0f;
        case T_EMO_CURIOSITY: return 3.0f;
        case T_EMO_CRAVING: return 3.0f;
        case T_EMO_EXCITEMENT: return 3.0f;
        case T_EMO_FEAR: return 5.0f;
        case T_EMO_JOY: return 6.0f;
        case T_EMO_LUST: return 5.0f;
        case T_EMO_PRIDE: return 6.0f;
        case T_EMO_SADNESS: return 8.0f;
        case T_EMO_DISAPPOINTMENT: return 8.0f;
        case T_EMO_GRATITUDE: return 9.0f;
        case T_EMO_RELIEF: return 6.0f;
        case T_EMO_HOPE: return 12.0f;
        case T_EMO_DREAD: return 10.0f;
        case T_EMO_AFFECTION: return 20.0f;
        case T_EMO_TRUST: return 30.0f;
        case T_EMO_LONGING: return 25.0f;
        case T_EMO_COMPASSION: return 15.0f;
        case T_EMO_JEALOUSY: return 18.0f;
        case T_EMO_SHYNESS: return 12.0f;
        case T_EMO_BOREDOM: return 14.0f;
        default: return 10.0f; /* contentment */
    }
}

static inline int TDominantEmotion(TFLY *fly) {
    if (fly == NULL) return T_EMO_CONTENTMENT;
    int best = 0;
    float bv = -1.0f;
    for (int i = 0; i < TFLY_N_EMOTION; i++) {
        if (fly->emotion[i] > bv) { bv = fly->emotion[i]; best = i; }
    }
    return best;
}

static inline int TDominantDrive(TFLY *fly) {
    if (fly == NULL) return T_DRIVE_HUNGER;
    int best = 0;
    float bv = -1.0f;
    for (int i = 0; i < TFLY_N_DRIVE; i++) {
        if (fly->drive[i] > bv) { bv = fly->drive[i]; best = i; }
    }
    return best;
}

/* Total plasticity currently available, i.e. how receptive to learning. */
static inline float TLearningGate(TFLY *fly) {
    if (fly == NULL) return 0.0f;
    float dop = fly->hormone[T_H_DOPAMINE];
    float oct = fly->hormone[T_H_OCTOPAMINE];
    float ser = fly->hormone[T_H_SEROTONIN];
    float npf = fly->hormone[T_H_NEUROPEPTIDE_F];
    float dark = fly->sensory[T_DARK];
    return TClamp(0.15f + 0.55f * dop + 0.45f * oct + 0.25f * npf - 0.35f * ser + 0.20f * dark, 0.0f,
                  1.0f);
}

static inline const char *TFlyEmotionName(int e) {
    switch (TClampIdx(e, TFLY_N_EMOTION)) {
        case T_EMO_PAIN: return "pain";
        case T_EMO_FEAR: return "fear";
        case T_EMO_JOY: return "joy";
        case T_EMO_SADNESS: return "sadness";
        case T_EMO_ANGER: return "anger";
        case T_EMO_DISGUST: return "disgust";
        case T_EMO_SURPRISE: return "surprise";
        case T_EMO_CURIOSITY: return "curiosity";
        case T_EMO_LUST: return "lust";
        case T_EMO_CRAVING: return "craving";
        case T_EMO_CONTENTMENT: return "contentment";
        case T_EMO_DREAD: return "dread";
        case T_EMO_CONFUSION: return "confusion";
        case T_EMO_PRIDE: return "pride";
        case T_EMO_GRATITUDE: return "gratitude";
        case T_EMO_RELIEF: return "relief";
        case T_EMO_DISAPPOINTMENT: return "disappointment";
        case T_EMO_HOPE: return "hope";
        case T_EMO_JEALOUSY: return "jealousy";
        case T_EMO_SHYNESS: return "shyness";
        case T_EMO_AFFECTION: return "affection";
        case T_EMO_BOREDOM: return "boredom";
        case T_EMO_EXCITEMENT: return "excitement";
        case T_EMO_COMPASSION: return "compassion";
        case T_EMO_TRUST: return "trust";
        default: return "longing";
    }
}

static inline const char *TFlyDriveName(int d) {
    switch (TClampIdx(d, TFLY_N_DRIVE)) {
        case T_DRIVE_HUNGER: return "hunger";
        case T_DRIVE_THIRST: return "thirst";
        case T_DRIVE_SEX: return "sex";
        case T_DRIVE_SLEEP: return "sleep";
        case T_DRIVE_FATIGUE: return "fatigue";
        case T_DRIVE_COLD: return "cold";
        case T_DRIVE_SOCIAL: return "social";
        default: return "curiosity";
    }
}

static inline const char *TFlyHormoneName(int h) {
    switch (TClampIdx(h, TFLY_N_HORMONE)) {
        case T_H_OCTOPAMINE: return "octopamine";
        case T_H_DOPAMINE: return "dopamine";
        case T_H_SEROTONIN: return "serotonin";
        case T_H_NOVELTY: return "novelty";
        case T_H_INSULIN: return "insulin";
        case T_H_LEPTIN: return "leptin";
        case T_H_ECDYSONE: return "ecdysone";
        case T_H_JUVENILE_HORMONE: return "juvenile_hormone";
        case T_H_GLUTAMATE: return "glutamate";
        case T_H_GABA: return "gaba";
        case T_H_ACETYLCHOLINE: return "acetylcholine";
        case T_H_NITRIC_OXIDE: return "nitric_oxide";
        case T_H_CORAZONIN: return "corazonin";
        case T_H_NEUROPEPTIDE_F: return "neuropeptide_f";
        case T_H_DH31: return "dh31";
        case T_H_FMRFAMIDE: return "fmrfamide";
        case T_H_CCAP: return "ccap";
        case T_H_RELAXIN: return "relaxin";
        case T_H_ETH: return "eth";
        case T_H_IRS: return "irs";
        case T_H_TOR: return "tor";
        case T_H_SNPF: return "snpf";
        case T_H_NLPP: return "nlpp";
        case T_H_PAM: return "pam";
        case T_H_PTTH: return "ptth";
        default: return "dh44";
    }
}

static inline const char *TFlyActionName(int a) {
    switch (TClampIdx(a, TFLY_N_ACTION)) {
        case T_ACT_REST: return "rest";
        case T_ACT_FORWARD: return "forward";
        case T_ACT_TURN_L: return "turn_left";
        case T_ACT_TURN_R: return "turn_right";
        case T_ACT_UP: return "up";
        case T_ACT_DOWN: return "down";
        case T_ACT_FLAP: return "flap";
        case T_ACT_DANCE: return "dance";
        case T_ACT_EAT: return "eat";
        default: return "mate";
    }
}

static inline const char *TFlySensoryName(int s) {
    switch (TClampIdx(s, TFLY_N_SENSORY)) {
        case T_LIGHT: return "light";
        case T_DARK: return "dark";
        case T_ODOR: return "odor";
        case T_TOUCH: return "touch";
        case T_TEMPERATURE: return "temperature";
        case T_GRAVITY: return "gravity";
        case T_PROPRIOCEPTION: return "proprioception";
        case T_SOUND: return "sound";
        case T_VIBRATION: return "vibration";
        case T_TASTE: return "taste";
        case T_SMELL_ANTENNA: return "antenna";
        default: return "wind";
    }
}

/* ------------------------------------------------------------------ *
 * Gestures and encounters
 *
 * A gesture is a pose the rig reads; an encounter is what two flies do to
 * each other. Encounters are symmetric and have real consequences: they move
 * the bond in both directions and change both flies' affect. That is the
 * whole difference between two characters standing near each other and two
 * characters who know each other.
 * ------------------------------------------------------------------ */

static inline const char *TFlyGestureName(int g) {
    switch (TClampIdx(g, TFLY_N_GESTURE)) {
        case T_GES_WAVE: return "приветствие";
        case T_GES_BOW: return "поклон";
        case T_GES_CLAP: return "хлопки";
        case T_GES_POINT: return "указание";
        case T_GES_COVER: return "утешение";
        case T_GES_SHRUG: return "плечики";
        case T_GES_HOLD: return "прошу обнять";
        default: return "покой";
    }
}

static inline const char *TFlyEncounterName(int e) {
    switch (TClampIdx(e, TFLY_N_ENCOUNTER)) {
        case T_ENC_GREET: return "поздоровались";
        case T_ENC_BOW: return "поклонились";
        case T_ENC_HIGH_FIVE: return "похлопали ладони";
        case T_ENC_COMFORT: return "утешили";
        case T_ENC_SHARE: return "поделились";
        case T_ENC_ARGUE: return "поссорились";
        default: return "разошлись";
    }
}

/* Start a pose. A gesture already at full strength is not interrupted, so an
 * encounter cannot cut a bow off halfway through. */
static inline void TGesture(TFLY *fly, int g) {
    if (fly == NULL) return;
    int id = TClampIdx(g, TFLY_N_GESTURE);
    if (id == T_GES_IDLE) return;
    if (fly->gesture != T_GES_IDLE && fly->gesture_strength > 0.5f) return;
    fly->gesture = id;
    fly->gesture_phase = 0.0f;
    fly->gesture_strength = 0.0f;
}

/* Start a pose, interrupting whatever was running.
 *
 * Used for direct control. The guarded version above would silently drop the
 * request if a previous pose were still at full strength, which is the right
 * behaviour for an automatic encounter and the wrong one for a button press. */
static inline void TGestureForce(TFLY *fly, int g) {
    if (fly == NULL) return;
    fly->gesture = TClampIdx(g, TFLY_N_GESTURE);
    fly->gesture_phase = 0.0f;
    fly->gesture_strength = 0.0f;
}

/* Can this fly start an encounter right now? */
static inline int TCanEncounter(TFLY *fly) {
    return fly != NULL && fly->encounter_cool <= 0.0f;
}

/*
 * Two flies meet. The pair is symmetric: both feel the same thing and both
 * gain or lose bond, because an encounter that moved only one side would not
 * be a relationship.
 *
 * Returns 1 if the encounter happened. Cooldowns are respected, so a pair
 * standing together does not repeat the same greeting over and over.
 */
static inline int TEncounter(TFLY *a, TFLY *b, int kind) {
    if (a == NULL || b == NULL) return 0;
    if (!TCanEncounter(a) || !TCanEncounter(b)) return 0;

    int e = TClampIdx(kind, TFLY_N_ENCOUNTER);
    /* A quarrel takes longer to get over than a greeting. */
    float cool = (e == T_ENC_ARGUE) ? 9.0f : ((e == T_ENC_IGNORE) ? 3.0f : 5.0f);
    a->encounter_cool = cool;
    b->encounter_cool = cool;
    a->last_encounter = e;
    b->last_encounter = e;

    float bond_delta = 0.0f;
    switch (e) {
        case T_ENC_GREET:     bond_delta = 0.08f;  break;
        case T_ENC_BOW:       bond_delta = 0.06f;  break;
        case T_ENC_HIGH_FIVE: bond_delta = 0.10f;  break;
        case T_ENC_COMFORT:   bond_delta = 0.14f;  break;
        case T_ENC_SHARE:     bond_delta = 0.18f;  break;
        case T_ENC_ARGUE:     bond_delta = -0.12f; break;
        default:              bond_delta = -0.01f; break;
    }
    /* Warmth scales how much a positive encounter means, so a cold fly gets
     * less out of being greeted than an affectionate one. */
    float warmth = TClamp(0.5f + 0.5f * (a->emotion[T_EMO_AFFECTION] + b->emotion[T_EMO_AFFECTION]),
                          0.2f, 1.2f);
    a->bond = TClamp(a->bond + bond_delta * warmth, 0.0f, 1.0f);
    b->bond = TClamp(b->bond + bond_delta * warmth, 0.0f, 1.0f);

    /* Poses, so the rig has something to show. */
    switch (e) {
        case T_ENC_GREET:     TGesture(a, T_GES_WAVE);  TGesture(b, T_GES_WAVE);  break;
        case T_ENC_BOW:       TGesture(a, T_GES_BOW);   TGesture(b, T_GES_BOW);   break;
        case T_ENC_HIGH_FIVE: TGesture(a, T_GES_CLAP);  TGesture(b, T_GES_CLAP);  break;
        case T_ENC_COMFORT:   TGesture(a, T_GES_COVER); TGesture(b, T_GES_COVER); break;
        case T_ENC_SHARE:     TGesture(a, T_GES_POINT); TGesture(b, T_GES_POINT); break;
        case T_ENC_ARGUE:     TGesture(a, T_GES_SHRUG); TGesture(b, T_GES_SHRUG); break;
        default:              TGesture(a, T_GES_IDLE);  TGesture(b, T_GES_IDLE);  break;
    }

    /* Affect. Both feel it, with a little asymmetry so a pair does not mirror
     * each other perfectly. */
    float j, s, f, t, an;
    switch (e) {
        case T_ENC_GREET:     j = 0.30f; s = 0.10f; f = 0.00f; t = 0.20f; an = 0.00f; break;
        case T_ENC_BOW:       j = 0.20f; s = 0.05f; f = 0.00f; t = 0.30f; an = 0.00f; break;
        case T_ENC_HIGH_FIVE: j = 0.40f; s = 0.05f; f = 0.00f; t = 0.25f; an = 0.00f; break;
        case T_ENC_COMFORT:   j = 0.25f; s = 0.05f; f = 0.00f; t = 0.40f; an = 0.00f; break;
        case T_ENC_SHARE:     j = 0.45f; s = 0.00f; f = 0.00f; t = 0.50f; an = 0.00f; break;
        case T_ENC_ARGUE:     j = 0.00f; s = 0.25f; f = 0.20f; t = 0.00f; an = 0.45f; break;
        default:              j = 0.00f; s = 0.05f; f = 0.00f; t = 0.00f; an = 0.00f; break;
    }
    float side = 0.9f + 0.2f * TRand(a);
    TFLY *pair[2];
    pair[0] = a;
    pair[1] = b;
    for (int i = 0; i < 2; i++) {
        TFLY *who = pair[i];
        float k = (i == 0) ? side : (1.9f - side);
        who->emotion[T_EMO_JOY] = TClamp(who->emotion[T_EMO_JOY] + j * k, 0.0f, 1.0f);
        who->emotion[T_EMO_SADNESS] = TClamp(who->emotion[T_EMO_SADNESS] + s * k, 0.0f, 1.0f);
        who->emotion[T_EMO_FEAR] = TClamp(who->emotion[T_EMO_FEAR] + f * k, 0.0f, 1.0f);
        who->emotion[T_EMO_TRUST] = TClamp(who->emotion[T_EMO_TRUST] + t * k, 0.0f, 1.0f);
        who->emotion[T_EMO_ANGER] = TClamp(who->emotion[T_EMO_ANGER] + an * k, 0.0f, 1.0f);
        who->emotion[T_EMO_AFFECTION] =
            TClamp(who->emotion[T_EMO_AFFECTION] + t * 0.6f * k, 0.0f, 1.0f);
    }
    return 1;
}

/*
 * How likely a fly is to start an encounter, 0..1. Affection and longing pull
 * her toward company, shyness holds her back, and anger ruins it entirely.
 */
static inline float TEncounterDrive(TFLY *fly) {
    if (fly == NULL) return 0.0f;
    if (fly->encounter_cool > 0.0f) return 0.0f;
    float social = fly->emotion[T_EMO_AFFECTION] + fly->emotion[T_EMO_LONGING];
    float shy = fly->emotion[T_EMO_SHYNESS];
    /* Fear is a hard brake, because a frightened animal hides rather than
     * introducing itself. Without this term a badly hurt fly would still be
     * walking toward whoever hurt her. */
    float fear = fly->emotion[T_EMO_FEAR] + fly->emotion[T_EMO_DREAD];
    float raw = social * (1.0f - 0.5f * shy) - 0.5f * fly->emotion[T_EMO_ANGER] -
                1.2f * fear;
    return TClamp(raw, 0.0f, 1.0f);
}

static inline const char *TFlyBodyName(int r) {
    switch (TClampIdx(r, TFLY_N_REGION)) {
        case T_BODY_HEAD: return "head";
        case T_BODY_THORAX: return "thorax";
        case T_BODY_ABDOMEN: return "abdomen";
        case T_BODY_WING_L: return "wing_l";
        case T_BODY_WING_R: return "wing_r";
        case T_BODY_LEG_L: return "leg_l";
        case T_BODY_LEG_R: return "leg_r";
        case T_BODY_EYE_L: return "eye_l";
        case T_BODY_EYE_R: return "eye_r";
        default: return "proboscis";
    }
}

/* ------------------------------------------------------------------ *
 * Reporting
 * ------------------------------------------------------------------ */

static inline void TPrintState(const TFLY *fly) {
    if (fly == NULL) return;
    printf("t=%.2fs tick=%u\n", (double)fly->t, fly->ticks);
    printf("  mood      %s  valence=%+.3f arousal=%.3f\n", TFlyEmotionName(TDominantEmotion((TFLY *)fly)),
           (double)fly->valence, (double)fly->arousal);
    printf("  drive     %s=%.3f\n", TFlyDriveName(TDominantDrive((TFLY *)fly)),
           (double)fly->drive[TDominantDrive((TFLY *)fly)]);
    printf("  pain      total=%.3f memory=%.3f\n", (double)fly->pain_total, (double)fly->pain_memory);
    printf("  motor     thrust=%.3f turn=%+.3f vertical=%+.3f wingbeat=%.1fHz\n", (double)fly->out_thrust,
           (double)fly->out_turn, (double)fly->out_vertical, (double)fly->out_wingbeat);
    printf("  physiology energy=%.3f stress=%.3f lifespan=%.3f\n", (double)fly->energy, (double)fly->stress,
           (double)fly->lifespan);
    printf("  learning  gate=%.3f plasticity=%.3f\n", (double)TLearningGate((TFLY *)fly),
           (double)fly->plasticity);
}

static inline void TPrintHormones(const TFLY *fly) {
    if (fly == NULL) return;
    printf("  hormones\n");
    for (int h = 0; h < TFLY_N_HORMONE; h++) {
        if (fly->hormone[h] > 0.01f) printf("    %-18s %.3f\n", TFlyHormoneName(h), (double)fly->hormone[h]);
    }
}

static inline void TPrintEmotions(const TFLY *fly) {
    if (fly == NULL) return;
    printf("  emotions\n");
    for (int e = 0; e < TFLY_N_EMOTION; e++) {
        if (fly->emotion[e] > 0.01f) printf("    %-12s %.3f\n", TFlyEmotionName(e), (double)fly->emotion[e]);
    }
}

/* Compact JSON state, convenient for the editor and for logging. */
static inline int TToJson(const TFLY *fly, char *buf, int cap) {
    if (fly == NULL || buf == NULL || cap <= 0) return 0;
    int n = snprintf(buf, (size_t)cap,
                     "{\"t\":%.4f,\"ticks\":%u,\"mood\":\"%s\",\"valence\":%.4f,\"arousal\":%.4f,"
                     "\"thrust\":%.4f,\"turn\":%.4f,\"vertical\":%.4f,\"wingbeat\":%.2f,"
                     "\"energy\":%.4f,\"stress\":%.4f,\"pain\":%.4f,\"lifespan\":%.4f,"
                     "\"plasticity\":%.4f,\"learning_gate\":%.4f,\"asleep\":%d}",
                     (double)fly->t, fly->ticks, TFlyEmotionName(TDominantEmotion((TFLY *)fly)),
                     (double)fly->valence, (double)fly->arousal, (double)fly->out_thrust, (double)fly->out_turn,
                     (double)fly->out_vertical, (double)fly->out_wingbeat, (double)fly->energy,
                     (double)fly->stress, (double)fly->pain_total, (double)fly->lifespan,
                     (double)fly->plasticity, (double)TLearningGate((TFLY *)fly), fly->asleep);
    return n < cap ? n : cap - 1;
}

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* TFLY_H */
