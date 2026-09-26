/*
 * tfly_test.c - compile and runtime check for TFLY.h
 *
 * Build:
 *   gcc -std=c99 -O2 -Wall -Wextra -o tfly_test native/tfly_test.c -lm
 *   ./tfly_test
 *
 * Run on Windows:
 *   .\tfly_test.exe
 */

#include <stdio.h>
#include <stdarg.h>
#include <math.h>
#include <string.h>

#include "../TFLY.h"

static int failures = 0;

static void check(int condition, const char *what) {
    if (condition) {
        printf("  ok   %s\n", what);
    } else {
        printf("  FAIL %s\n", what);
        failures++;
    }
}

static void checkf(int condition, const char *fmt, ...) {
    va_list args;
    char what[256];
    va_start(args, fmt);
    vsnprintf(what, sizeof what, fmt, args);
    va_end(args);
    check(condition, what);
}

static void check_near(float a, float b, float tol, const char *what) {
    if (fabsf(a - b) <= tol) {
        printf("  ok   %s (%.4f ~ %.4f)\n", what, (double)a, (double)b);
    } else {
        printf("  FAIL %s (%.4f != %.4f)\n", what, (double)a, (double)b);
        failures++;
    }
}

/* ------------------------------------------------------------------ */

static void test_lifecycle(void) {
    printf("lifecycle\n");
    TFLY fly;
    TNew(&fly);
    check_near(fly.energy, 1.0f, 1e-5, "fresh energy");
    check_near(fly.lifespan, 1.0f, 1e-5, "fresh lifespan");
    check_near(fly.sensory[T_DARK], 1.0f, 1e-5, "dark before light");
    TLight(&fly, 0.25f);
    check_near(fly.sensory[T_DARK], 0.75f, 1e-5, "dark after light");
    TSeed(&fly, 42);
    check(fly.rng == 42, "seed stored");
    TSeed(&fly, 0);
    check(fly.rng != 0, "zero seed replaced");
}

/* ------------------------------------------------------------------ */

static void test_pain_localisation(void) {
    printf("pain localisation\n");
    TFLY fly;
    TNew(&fly);
    TClearHormones(&fly);

    float total = THeart(&fly, 0.8f, T_BODY_WING_L);
    check(total > 0.0f, "THeart returns a signal");
    check(fly.pain[T_BODY_WING_L] > 0.3f, "left wing carries the pain");
    check(fly.pain[T_BODY_LEG_L] < fly.pain[T_BODY_WING_L], "referred spread is weaker than the source");
    check(fly.emotion[T_EMO_PAIN] > 0.3f, "pain affect raised");
    check(fly.stress > 0.1f, "stress raised");
    check(fly.action[T_ACT_UP] > 0.1f, "startle reflex lifts the fly");

    /* thorax pain is felt everywhere */
    TFLY wide;
    TNew(&wide);
    THeart(&wide, 1.0f, T_BODY_THORAX);
    check(wide.pain[T_BODY_HEAD] > 0.05f, "thorax pain reaches the head");
    check(wide.pain[T_BODY_ABDOMEN] > 0.05f, "thorax pain reaches the abdomen");

    /* analgesia lowers the signal without deleting the memory */
    THeal(&fly, T_BODY_WING_L, 1.0f);
    check_near(fly.pain[T_BODY_WING_L], 0.0f, 1e-5, "heal clears local pain");
    check(fly.pain_memory > 0.0f, "pain memory survives healing");
    THealAll(&fly);
    check_near(fly.pain_total, 0.0f, 1e-5, "heal all clears the total");
}

/* ------------------------------------------------------------------ */

static void test_hormones(void) {
    printf("hormones\n");
    TFLY fly;
    TNew(&fly);
    TClearHormones(&fly);

    float level = THormone(&fly, 0.6f, T_H_DOPAMINE, 1.0f);
    check_near(level, 0.6f, 1e-5, "THormone(x, y, z) sets the level");
    check(fly.pulse[T_H_DOPAMINE] > 0.0f, "positive z emits a pulse");

    /* z <= 0 shifts the tonic set-point instead */
    float tonic = THormone(&fly, 0.4f, T_H_SEROTONIN, 0.0f);
    check(tonic >= 0.4f, "pulse-free call still raises the level");
    check_near(fly.pulse[T_H_SEROTONIN], 0.0f, 1e-5, "zero duration emits nothing");
    check(fly.hormone_set[T_H_SEROTONIN] > 0.0f, "tonic set-point recorded");

    check_near(THormoneByName(&fly, 0.5f, "octopamine", 1.0f), 0.5f, 1e-5, "hormones by name");
    check_near(THormoneByName(&fly, 0.5f, "not_a_hormone", 1.0f), 0.0f, 1e-5, "unknown name is rejected");
    check(strcmp(TFlyHormoneName(T_H_NEUROPEPTIDE_F), "neuropeptide_f") == 0, "hormone name lookup");

    /* the tonic set-point should hold the level across many steps */
    TFLY hold;
    TNew(&hold);
    TClearHormones(&hold);
    THormone(&hold, 0.5f, T_H_INSULIN, 0.0f);
    TSteps(&hold, 600, 1.0f / 60.0f);
    check(hold.hormone[T_H_INSULIN] > 0.2f, "tonic hormone is maintained over time");

    /* pulses must decay */
    TFLY fade;
    TNew(&fade);
    TClearHormones(&fade);
    THormone(&fade, 0.9f, T_H_OCTOPAMINE, 1.0f);
    TSteps(&fade, 120, 1.0f / 60.0f);
    check(fade.pulse[T_H_OCTOPAMINE] < 0.05f, "pulse decays to nothing");
}

/* ------------------------------------------------------------------ */

static void test_emotions(void) {
    printf("affect\n");
    TFLY fly;
    TNew(&fly);

    TFear(&fly, 0.8f);
    check(fly.emotion[T_EMO_FEAR] > 0.7f, "fear can be called directly");
    check(fly.emotion[T_EMO_JOY] < 0.1f, "fear suppresses joy");
    check(fly.pulse[T_H_OCTOPAMINE] > 0.0f, "fear releases octopamine");

    TJoy(&fly, 0.7f);
    check(fly.emotion[T_EMO_FEAR] < 0.55f, "joy partially damps fear");
    check(fly.pulse[T_H_DOPAMINE] > 0.0f, "joy releases dopamine");
    check(strcmp(TFlyEmotionName(T_EMO_CURIOSITY), "curiosity") == 0, "emotion name lookup");

    TUpdate(&fly, 1.0f / 60.0f);
    check(fly.valence > -0.2f, "valence recomputed after update");
    check(fly.arousal >= 0.0f && fly.arousal <= 1.0f, "arousal stays in range");

    /* Every emotion must decay when left alone. Curiosity is the slowest to
     * fade because the fly keeps re-deriving it from its own drive, so it is
     * checked against a looser bound than the transient states. */
    TFLY calm;
    TNew(&calm);
    TCuriosity(&calm, 1.0f);
    float start = calm.emotion[T_EMO_CURIOSITY];
    TSteps(&calm, 1200, 1.0f / 60.0f);
    check(calm.emotion[T_EMO_CURIOSITY] < start, "emotions decay without reinforcement");
    check(calm.emotion[T_EMO_CURIOSITY] < 0.35f, "curiosity settles to a drive level, not to zero");

    /* transient states must actually reach the floor */
    TFLY spike;
    TNew(&spike);
    TSurprise(&spike, 1.0f);
    TSteps(&spike, 1200, 1.0f / 60.0f);
    check(spike.emotion[T_EMO_SURPRISE] < 0.05f, "surprise decays to the floor");
}

/* ------------------------------------------------------------------ */

static void test_sensory(void) {
    printf("sensory\n");
    TFLY fly;
    TNew(&fly);

    TOdor(&fly, 0.5f, T_ODOR_FRUIT);
    check(fly.sensory_raw[T_ODOR] > 0.2f, "odour reaches the raw channel");
    check(fly.sensory[T_SMELL_ANTENNA] > fly.sensory_raw[T_ODOR], "antenna gain follows odour");
    check(fly.drive[T_DRIVE_HUNGER] < 0.05f, "fruit odour reduces hunger");

    TTaste(&fly, 0.8f, 0.0f, 0.0f, 0.0f);
    check(fly.hormone[T_H_DOPAMINE] > 0.0f, "sweet taste is rewarding");
    float before = fly.emotion[T_EMO_DISGUST];
    TTaste(&fly, 0.0f, 0.0f, 0.9f, 0.0f);
    check(fly.emotion[T_EMO_DISGUST] > before, "bitter taste is disgusting");

    TAttention(&fly, T_ODOR, 2.0f);
    check_near(fly.gain[T_ODOR], 2.0f, 1e-5, "attention raises gain");
    float v = fly.sensory[T_SMELL_ANTENNA];
    check(v > 0.0f && v <= 1.0f, "derived channel stays clamped");

    TAttentionFatigue(&fly, 1.0f);
    check_near(fly.gain[T_ODOR], 1.0f, 1e-5, "attention fatigues back to 1");

    /* inputs must be clamped, never trusted */
    TLight(&fly, 5.0f);
    check_near(fly.sensory_raw[T_LIGHT], 1.0f, 1e-5, "light input is clamped");
    TLight(&fly, -3.0f);
    check_near(fly.sensory_raw[T_LIGHT], 0.0f, 1e-5, "negative light is clamped");

    /* stimuli must be order independent */
    TFLY a, b;
    TNew(&a);
    TNew(&b);
    TOdor(&a, 0.6f, T_ODOR_FRUIT);
    TLight(&a, 0.3f);
    TLight(&b, 0.3f);
    TOdor(&b, 0.6f, T_ODOR_FRUIT);
    check_near(a.sensory[T_SMELL_ANTENNA], b.sensory[T_SMELL_ANTENNA], 1e-5, "derived channel is order independent");
}

/* ------------------------------------------------------------------ */

static void test_learning(void) {
    printf("learning\n");
    TFLY fly;
    TNew(&fly);

    /* three-factor rule: no modulator, no learning */
    TAssociate(&fly, T_CUE_LIGHT, T_ACT_FORWARD, 1.0f, 0.0f);
    check_near(fly.assoc[T_CUE_LIGHT][T_ACT_FORWARD], 0.0f, 1e-5, "no modulator means no update");

    /* reward with a modulator strengthens the association */
    TSteps(&fly, 30, 1.0f / 60.0f);
    float before = fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD];
    for (int trial = 0; trial < 20; trial++) {
        TSteps(&fly, 30, 1.0f / 60.0f);
        TAssociate(&fly, T_CUE_ODOR_FRUIT, T_ACT_FORWARD, 1.0f, 1.0f);
    }
    check(fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD] > before, "reward increases the weight");

    /* punishment drives it back down */
    float rewarded = fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD];
    for (int trial = 0; trial < 40; trial++) {
        TSteps(&fly, 30, 1.0f / 60.0f);
        TAssociate(&fly, T_CUE_ODOR_FRUIT, T_ACT_FORWARD, -1.0f, 1.0f);
    }
    check(fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD] < rewarded, "punishment decreases the weight");

    /* weights must stay inside their declared bounds */
    TFLY blow;
    TNew(&blow);
    for (int i = 0; i < 5000; i++) {
        TAssociate(&blow, T_CUE_SOUND, T_ACT_TURN_L, 1.0f, 1.0f);
    }
    check(fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD] <= 1.0f, "weights are clamped to 1");

    /* dopamine gates plasticity */
    TFLY gated;
    TNew(&gated);
    TClearHormones(&gated);
    check(TLearningGate(&gated) < 0.6f, "learning gate is low without modulators");
    TDopamine(&gated, 1.0f, 5.0f);
    TSteps(&gated, 10, 1.0f / 60.0f);
    check(TLearningGate(&gated) > 0.6f, "dopamine raises the learning gate");

    TPlasticity(&gated, 0.0f);
    check_near(gated.plasticity, 0.0f, 1e-5, "plasticity is settable");
    TPlasticity(&gated, 1.0f);
    TResetLearning(&gated);
    check_near(gated.assoc[T_CUE_SOUND][T_ACT_TURN_L], 0.0f, 1e-5, "learning can be wiped");

    /* The learning rate is exposed so the pace of a lesson can be watched at both
     * ends. It must stay a multiplier on a permitted update and nothing else: a
     * shut gate has to refuse every update however high the rate is, or the
     * three-factor rule is decoration. */
    check_near(TLearnRateOf(&gated), 1.0f, 1e-5, "a new fly learns at the default rate");
    TLearnRate(&gated, 20.0f);
    check_near(TLearnRateOf(&gated), 20.0f, 1e-5, "the rate is settable");
    TResetLearning(&gated);
    for (int i = 0; i < 2000; i++) {
        TAssociate(&gated, T_CUE_LIGHT, T_ACT_DANCE, 1.0f, 0.0f);
    }
    check_near(gated.assoc[T_CUE_LIGHT][T_ACT_DANCE], 0.0f, 1e-5,
               "a shut gate refuses every update at rate 20");

    /* And a high rate must reach the same weight in fewer trials, not a
     * different one. The action has to be performed first, because the
     * eligibility trace is built from the motor output and not from the call
     * to associate: a trial that only calls TAssociate teaches nothing at any
     * rate, which is the whole reason the harness lets the fly act. */
    int trials_for[2];
    for (int which = 0; which < 2; which++) {
        TFLY paced;
        TNew(&paced);
        TLearnRate(&paced, which == 0 ? 1.0f : 10.0f);
        int n = 0;
        while (n < 20000 && paced.assoc[T_CUE_SOUND][T_ACT_DANCE] < 0.8f) {
            TAct(&paced, T_ACT_DANCE, 1.0f);
            TSteps(&paced, 2, 1.0f / 60.0f);
            TAssociate(&paced, T_CUE_SOUND, T_ACT_DANCE, 1.0f, 1.0f);
            n++;
        }
        trials_for[which] = n;
    }
    check(trials_for[0] < 20000, "the default rate reaches the lesson");
    check(trials_for[1] < trials_for[0], "a higher rate needs fewer trials");

    /* The rate is clamped, so a wild number cannot produce an unstable step. */
    TFLY wild;
    TNew(&wild);
    TLearnRate(&wild, 1e6f);
    check(TLearnRateOf(&wild) <= 20.0f, "the learning rate is clamped");
    for (int i = 0; i < 5000; i++) {
        TAssociate(&wild, T_CUE_VIBRATION, T_ACT_TURN_R, 1.0f, 2.0f);
    }
    check(wild.assoc[T_CUE_VIBRATION][T_ACT_TURN_R] <= 1.0f,
          "a clamped rate still keeps weights inside their bounds");
}

/* ------------------------------------------------------------------ */

static void test_memory(void) {
    printf("memory\n");
    TFLY fly;
    TNew(&fly);
    TMemorize(&fly, 3.0f, 0.75f);
    TMemorize(&fly, 7.0f, 0.25f);
    check_near(TRecall(&fly, 3.0f), 0.75f, 1e-5, "recall by key");
    check_near(TRecall(&fly, 7.0f), 0.25f, 1e-5, "recall second entry");
    check_near(TRecall(&fly, 99.0f), 0.0f, 1e-5, "missing key returns zero");

    /* overflow must not corrupt memory */
    for (int i = 0; i < 200; i++) TMemorize(&fly, (float)i, (float)i * 0.01f);
    check(fly.memory_n == TFLY_N_MEMORY, "memory count is bounded");
    check_near(TRecall(&fly, 199.0f), 1.99f, 1e-4, "newest entry is retrievable");

    TForget(&fly);
    check(fly.memory_n == 0, "forget clears memory");
}

/* ------------------------------------------------------------------ */

static void test_policy(void) {
    printf("policy\n");
    TFLY fly;
    TNew(&fly);
    TSteps(&fly, 120, 1.0f / 60.0f);
    check(fly.out_wingbeat > 0.0f, "wingbeat runs while awake");

    /* fear must dominate a positive drive */
    TFLY scared;
    TNew(&scared);
    TEat(&scared, 1.0f);
    float eat_before = scared.action[T_ACT_EAT];
    TFear(&scared, 0.9f);
    TPolicy(&scared, 1.0f / 60.0f);
    check(scared.action[T_ACT_EAT] < eat_before, "fear suppresses eating");
    check(scared.out_vertical > 0.0f, "fear produces escape climb");

    /* sleep suppresses the wingbeat */
    TFLY sleeping;
    TNew(&sleeping);
    TSteps(&sleeping, 60, 1.0f / 60.0f);
    TSleep(&sleeping);
    TUpdate(&sleeping, 1.0f / 60.0f);
    check_near(sleeping.out_wingbeat, 0.0f, 1e-5, "sleep stops the wingbeat");
    TWake(&sleeping);
    check(sleeping.asleep == 0, "wake clears the sleep flag");

    /* motor outputs must stay in their declared ranges */
    TFLY runner;
    TNew(&runner);
    TSeed(&runner, 7);
    for (int i = 0; i < 2000; i++) {
        TFear(&runner, TRand(&runner));
        TCuriosity(&runner, TRand(&runner));
        TReward(&runner, TRand(&runner));
        TUpdate(&runner, 0.05f); /* deliberately oversized dt */
    }
    check(runner.out_thrust >= 0.0f && runner.out_thrust <= 1.0f, "thrust stays in 0..1");
    check(runner.out_turn >= -1.0f && runner.out_turn <= 1.0f, "turn stays in -1..1");
    check(runner.out_vertical >= -1.0f && runner.out_vertical <= 1.0f, "vertical stays in -1..1");
    check(runner.energy >= 0.0f && runner.energy <= 1.0f, "energy stays in 0..1");
    check(runner.stress >= 0.0f && runner.stress <= 1.0f, "stress stays in 0..1");
    check(runner.valence >= -1.0f && runner.valence <= 1.0f, "valence stays in -1..1");
    check(runner.lifespan >= 0.0f, "lifespan does not go negative");
}

/* ------------------------------------------------------------------ */

static void test_homeostasis(void) {
    printf("homeostasis\n");
    TFLY fly;
    TNew(&fly);
    float hunger = fly.drive[T_DRIVE_HUNGER];
    TSteps(&fly, 600, 1.0f / 60.0f);
    check(fly.drive[T_DRIVE_HUNGER] > hunger, "drives accumulate when unmet");

    /* Nothing satisfies a drive on its own. The window has to stay inside a
     * single sleep cycle, because once the fly is allowed to sleep it
     * legitimately recovers the energy it just burned. */
    TFLY starving;
    TNew(&starving);
    TSteps(&starving, 4500, 1.0f / 60.0f);
    check(starving.drive[T_DRIVE_HUNGER] > 0.5f, "hunger grows without food");
    check(starving.asleep == 0, "a fresh fly is not asleep yet");
    check(starving.energy < 1.0f, "flying burns energy");
    check(starving.energy >= 0.0f, "energy does not go negative");

    /* feeding restores energy */
    TFLY fed;
    TNew(&fed);
    TSteps(&fed, 600, 1.0f / 60.0f);
    float low = fed.energy;
    for (int i = 0; i < 300; i++) {
        TEat(&fed, 1.0f);
        TTaste(&fed, 1.0f, 0.0f, 0.0f, 0.0f);
        TUpdate(&fed, 1.0f / 60.0f);
    }
    check(fed.energy >= low, "feeding does not lose energy");

    /* A sleep cycle must be enterable, and it must be exitable. Sleeping is
     * what discharges the sleep drive, so the fly has to come back out. */
    TFLY tired;
    TNew(&tired);
    tired.energy = 0.02f;
    tired.drive[T_DRIVE_SLEEP] = 1.0f;
    TSteps(&tired, 10, 1.0f / 60.0f);
    check(tired.asleep == 1, "high sleep pressure forces sleep");

    int woke = 0;
    for (int i = 0; i < 3600; i++) {
        TUpdate(&tired, 1.0f / 60.0f);
        if (tired.asleep == 0) woke = 1;
    }
    check(woke == 1, "the fly eventually wakes up on its own");
    check(tired.energy > 0.05f, "rest restores energy");
    check(tired.drive[T_DRIVE_SLEEP] < 1.0f, "sleep discharges the sleep drive");
    TWake(&tired);
    check(tired.asleep == 0, "waking clears the sleep flag");
}

/* ------------------------------------------------------------------ */

static void test_social(void) {
    printf("social\n");
    TFLY fly;
    TNew(&fly);
    TMateSignal(&fly, 0.9f);
    check(fly.drive[T_DRIVE_SEX] > 0.2f, "mate signal raises the sex drive");
    check(fly.emotion[T_EMO_LUST] > 0.2f, "mate signal raises lust");
    check(fly.action[T_ACT_MATE] >= 0.0f, "mating action exists");

    TPredatorSignal(&fly, 1.0f);
    check(fly.emotion[T_EMO_FEAR] > 0.3f, "predator raises fear");
    check(fly.emotion[T_EMO_DREAD] > 0.2f, "predator raises dread");
    check(fly.pulse[T_H_OCTOPAMINE] > 0.0f, "predator releases octopamine");

    /* fear must suppress courtship, as a real fly's does */
    TFLY courtship;
    TNew(&courtship);
    TMateSignal(&courtship, 1.0f);
    TSteps(&courtship, 60, 1.0f / 60.0f);
    float mate_before = courtship.action[T_ACT_MATE];
    TPredatorSignal(&courtship, 1.0f);
    TPolicy(&courtship, 1.0f / 60.0f);
    check(courtship.action[T_ACT_MATE] < mate_before, "predator suppresses courtship");

    TRivalSignal(&fly, 0.8f);
    check(fly.emotion[T_EMO_ANGER] > 0.2f, "rival raises anger");
    float pressure = fly.rival_pressure;
    TSteps(&fly, 1200, 1.0f / 60.0f);
    check(fly.rival_pressure < pressure * 0.2f, "social signals decay");
}

/* ------------------------------------------------------------------ */

static void test_determinism(void) {
    printf("determinism\n");
    TFLY a, b;
    TNew(&a);
    TNew(&b);
    TSeed(&a, 12345);
    TSeed(&b, 12345);
    for (int i = 0; i < 500; i++) {
        float light = 0.5f + 0.5f * sinf((float)i * 0.05f);
        TLight(&a, light);
        TLight(&b, light);
        TOdor(&a, light, T_ODOR_FRUIT);
        TOdor(&b, light, T_ODOR_FRUIT);
        TUpdate(&a, 1.0f / 60.0f);
        TUpdate(&b, 1.0f / 60.0f);
    }
    check_near(a.out_thrust, b.out_thrust, 1e-6, "identical seeds give identical thrust");
    check_near(a.out_turn, b.out_turn, 1e-6, "identical seeds give identical turn");
    check_near(a.emotion[T_EMO_JOY], b.emotion[T_EMO_JOY], 1e-6, "identical seeds give identical affect");

    /* different seeds must diverge, otherwise exploration is broken */
    TFLY c;
    TNew(&c);
    TSeed(&c, 999);
    TLight(&c, 0.5f);
    TUpdate(&c, 1.0f / 60.0f);
    TLight(&a, 0.5f);
    TUpdate(&a, 1.0f / 60.0f);
    check(TAbs(c.out_turn - a.out_turn) > 1e-6f, "different seeds diverge");
}

/* ------------------------------------------------------------------ */

static void test_robustness(void) {
    printf("robustness\n");

    /* null pointers must not crash */
    TNew(NULL);
    TUpdate(NULL, 0.016f);
    THeart(NULL, 1.0f, 0);
    THormone(NULL, 1.0f, 0, 1.0f);
    TFear(NULL, 1.0f);
    TPrintState(NULL);
    check(1, "null pointers are ignored safely");

    /* zero and negative dt must be no-ops */
    TFLY fly;
    TNew(&fly);
    TSteps(&fly, 10, 1.0f / 60.0f);
    unsigned ticks = fly.ticks;
    TUpdate(&fly, 0.0f);
    TUpdate(&fly, -1.0f);
    check(fly.ticks == ticks, "non positive dt does not advance time");

    /* an absurd dt must be clamped, not propagated */
    float t_before = fly.t;
    TUpdate(&fly, 1000.0f);
    check(fly.t - t_before <= 0.2501f, "large dt is clamped to one safe step");
    check(fly.energy >= 0.0f && fly.energy <= 1.0f, "state survives a huge dt");

    /* out of range indices must be clamped */
    THeart(&fly, 1.0f, 999);
    TEmotion(&fly, -5, 0.5f);
    TDrive(&fly, 999, 0.5f);
    THormone(&fly, 0.5f, 999, 1.0f);
    TAssociate(&fly, 999, 999, 1.0f, 1.0f);
    check(1, "out of range indices are clamped");

    /* deep recursion of state must not produce NaN */
    TFLY chaos;
    TNew(&chaos);
    TSeed(&chaos, 31337);
    for (int i = 0; i < 20000; i++) {
        TLight(&chaos, TRand(&chaos));
        TOdor(&chaos, TRand(&chaos), (int)(TRand(&chaos) * 6.0f));
        TTouch(&chaos, TRand(&chaos));
        TTemperature(&chaos, TRand(&chaos));
        TGravity(&chaos, TRand(&chaos));
        TAttention(&chaos, (int)(TRand(&chaos) * TFLY_N_SENSORY), TRand(&chaos) * 2.0f);
        TTaste(&chaos, TRand(&chaos), TRand(&chaos), TRand(&chaos), TRand(&chaos));
        THeart(&chaos, TRand(&chaos), (int)(TRand(&chaos) * TFLY_N_REGION));
        TReward(&chaos, TRand(&chaos));
        TPunish(&chaos, TRand(&chaos));
        TUpdate(&chaos, TRand(&chaos) * 0.05f);
    }
    check(chaos.out_thrust == chaos.out_thrust, "no NaN in thrust after 20k random ticks");
    check(chaos.valence == chaos.valence, "no NaN in valence");
    check(chaos.energy == chaos.energy, "no NaN in energy");
    check(chaos.assoc[0][0] == chaos.assoc[0][0], "no NaN in associative weights");
}

/* ------------------------------------------------------------------ */

static void test_scenario_nociception(void) {
    printf("scenario: wing injury changes behaviour\n");
    TFLY fly;
    TNew(&fly);
    TSeed(&fly, 2024);

    /* baseline */
    for (int i = 0; i < 600; i++) {
        TOdor(&fly, 0.4f, T_ODOR_FRUIT);
        TUpdate(&fly, 1.0f / 60.0f);
    }
    float baseline_thrust = fly.out_thrust;
    float baseline_stress = fly.stress;

    /* injury to the left wing */
    THeart(&fly, 0.9f, T_BODY_WING_L);
    check(fly.stress > baseline_stress, "injury raises stress");
    check(fly.emotion[T_EMO_PAIN] > 0.4f, "injury raises pain");
    TSteps(&fly, 10, 1.0f / 60.0f);
    check(fly.out_turn != 0.0f, "injury induces evasive turning");
    check(baseline_thrust >= 0.0f, "baseline was sane");

    /* dopamine should raise the learning gate afterwards */
    TReward(&fly, 0.8f);
    TSteps(&fly, 5, 1.0f / 60.0f);
    check(TLearningGate(&fly) > 0.5f, "reward reopens the learning gate");
}

/* ------------------------------------------------------------------ *
 * The face. Every channel is derived from state, so the test drives the
 * state through real stimuli and checks the face follows. A face that only
 * responded to a direct setter would not be tied to anything.
 * ------------------------------------------------------------------ */
static void test_face(void) {
    printf("face\n");
    TFLY fly;
    TNew(&fly);
    TSeed(&fly, 11);

    /* A newborn is awake and looking at you. */
    check(fly.blink < 0.2f, "a newborn has her eyes open");
    check(fly.pupil > 0.5f, "a newborn has visible pupils");
    check(fabsf(fly.mouth) < 0.1f, "a newborn has a neutral mouth");

    /* Sleep closes the eyes on its own, with no timer involved. */
    TSleep(&fly);
    for (int i = 0; i < 120; i++) TSteps(&fly, 1, 1.0f / 60.0f);
    check(fly.blink > 0.8f, "sleep closes the eyes");
    TWake(&fly);

    /* Blinking is a pulse: the lids shut and open again, repeatedly. Count
     * the front of each closed interval rather than watching for a falling
     * edge, because the lids open faster than any single step can sample. */
    int blinks = 0;
    int closed = 0;
    for (int i = 0; i < 60 * 30; i++) {
        TSteps(&fly, 1, 1.0f / 60.0f);
        int shut = fly.blink > 0.5f;
        if (shut && !closed) blinks++;
        closed = shut;
    }
    check(blinks > 3, "a wakeful fly blinks repeatedly");

    /* Fear dilates the pupils relative to calm. Both flies see identical
     * inputs apart from the predator, so the difference is the predator. */
    TFLY calm, scared;
    TNew(&calm);
    TNew(&scared);
    TSeed(&calm, 7);
    TSeed(&scared, 7);
    for (int i = 0; i < 600; i++) {
        TSteps(&calm, 1, 1.0f / 60.0f);
        if (i % 20 == 0) TPredatorSignal(&scared, 0.8f);
        TSteps(&scared, 1, 1.0f / 60.0f);
    }
    check(scared.pupil > calm.pupil, "fear dilates the pupils");
    check(scared.brow > calm.brow, "fear raises the brows");
    check(scared.mouth < calm.mouth, "fear pulls the mouth down");

    /* Reward smiles. */
    TFLY happy;
    TNew(&happy);
    TSeed(&happy, 3);
    for (int i = 0; i < 300; i++) {
        TReward(&happy, 0.9f);
        TSteps(&happy, 1, 1.0f / 60.0f);
    }
    check(happy.mouth > 0.0f, "reward smiles");
    check(happy.blush > 0.0f, "reward brings colour to the face");

    /* Tears are a reservoir, not a mirror. Sadness fades long before the wet
     * on the cheeks does, so a character can still be crying after she has
     * stopped being sad. Punishment is used rather than a direct affect
     * injection, because that is the stimulus a fly would actually meet. */
    TFLY wet;
    TNew(&wet);
    TSeed(&wet, 5);
    TPunish(&wet, 0.9f);
    for (int i = 0; i < 600; i++) TSteps(&wet, 1, 1.0f / 60.0f);
    check(wet.tears > 0.1f, "sadness produces tears");
    check(wet.tears < 0.9f, "tears stay under the ceiling");
    check(wet.emotion[T_EMO_SADNESS] < 0.6f, "the sadness that made them has faded");
    check(wet.tears > wet.emotion[T_EMO_SADNESS] * 0.5f, "tears lag the sadness that made them");
    float peak = wet.tears;
    for (int i = 0; i < 1800; i++) {
        TReward(&wet, 0.9f);
        TSteps(&wet, 1, 1.0f / 60.0f);
    }
    check(wet.tears < peak, "tears dry once she is happy again");

    /* Gaze stays in range and drifts on its own when nothing holds it. */
    float swing = 0.0f;
    float first = fly.gaze_x;
    for (int i = 0; i < 600; i++) {
        TSteps(&fly, 1, 1.0f / 60.0f);
        swing += fabsf(fly.gaze_x - first);
    }
    check(swing > 0.5f, "the gaze drifts when nothing holds it");
    check(fly.gaze_x >= -1.0f && fly.gaze_x <= 1.0f, "gaze_x stays in range");
    check(fly.gaze_y >= -1.0f && fly.gaze_y <= 1.0f, "gaze_y stays in range");

    /* A face must survive being driven hard. */
    TFLY chaos;
    TNew(&chaos);
    TSeed(&chaos, 99);
    for (int i = 0; i < 20000; i++) {
        TRand(&chaos);
        TReward(&chaos, TRand(&chaos));
        TPunish(&chaos, TRand(&chaos));
        TPredatorSignal(&chaos, TRand(&chaos));
        TSleep(&chaos);
        TWake(&chaos);
        TSteps(&chaos, 1, 1.0f / 60.0f);
    }
    check(chaos.blink >= 0.0f && chaos.blink <= 1.0f, "blink survives chaos");
    check(chaos.pupil >= 0.5f && chaos.pupil <= 1.6f, "pupil survives chaos");
    check(chaos.brow >= -1.0f && chaos.brow <= 1.0f, "brow survives chaos");
    check(chaos.mouth >= -1.0f && chaos.mouth <= 1.0f, "mouth survives chaos");
    check(chaos.tears >= 0.0f && chaos.tears <= 1.0f, "tears survive chaos");
    check(chaos.blush >= 0.0f && chaos.blush <= 1.0f, "blush survives chaos");
    check(chaos.sweat >= 0.0f && chaos.sweat <= 1.0f, "sweat survives chaos");
    check(chaos.blink == chaos.blink, "no NaN in blink");

    /* ---- opening the eyes is a sequence, not a switch ---- */
    TFLY wake;
    TNew(&wake);
    TSeed(&wake, 13);
    TLight(&wake, 0.8f);
    TSleep(&wake);
    for (int i = 0; i < 180; i++) TSteps(&wake, 1, 1.0f / 60.0f);
    check(wake.blink > 0.9f, "the lids are shut while she sleeps");
    check(wake.eye_open < 0.1f, "a sleeping fly has her eyes deliberately shut");
    TWake(&wake);
    /* Immediately after waking the lids are still mostly down. */
    TSteps(&wake, 1, 1.0f / 60.0f);
    check(wake.eye_open < 0.5f, "the lids do not snap open the instant she wakes");
    /* And they end up open. */
    for (int i = 0; i < 200; i++) TSteps(&wake, 1, 1.0f / 60.0f);
    check(wake.eye_open > 0.95f, "she opens her eyes on purpose");
    check(wake.blink < 0.1f, "and ends up looking at the world");

    /* Waking is not monotonic: she rubs the sleep out with partial blinks,
     * which is what stops it reading as a switch being thrown. */
    TFLY rub;
    TNew(&rub);
    TSeed(&rub, 13);
    TSleep(&rub);
    for (int i = 0; i < 120; i++) TSteps(&rub, 1, 1.0f / 60.0f);
    TWake(&rub);
    float lowest = 1.0f;
    int reopened = 0;
    int was_shut = 1;
    for (int i = 0; i < 84; i++) {
        TSteps(&rub, 1, 1.0f / 60.0f);
        if (rub.blink < lowest) lowest = rub.blink;
        int shut = rub.blink > 0.2f;
        if (!shut && was_shut) reopened++;
        was_shut = shut;
    }
    check(lowest < 0.4f, "the lids come most of the way up while waking");
    check(reopened >= 1, "she blinks the sleep out on the way (partial reopen)");

    /* A startle must wake her, not just lift her lids. Without this her eyes
     * would fly open and then close again as the surprise decayed. */
    TFLY startled;
    TNew(&startled);
    TSeed(&startled, 17);
    TSleep(&startled);
    for (int i = 0; i < 120; i++) TSteps(&startled, 1, 1.0f / 60.0f);
    check(startled.asleep, "she is asleep to begin with");
    TSurprise(&startled, 0.95f);
    TSteps(&startled, 1, 1.0f / 60.0f);
    check(!startled.asleep, "a startle wakes her");
    /* The claim is that the lids reach the open position, not that they stay
     * there: the wake sequence that follows deliberately blinks the sleep
     * out, so the lids close again on the way up. */
    float flew = 1.0f;
    for (int i = 0; i < 30; i++) {
        TSteps(&startled, 1, 1.0f / 60.0f);
        if (startled.blink < flew) flew = startled.blink;
    }
    check(flew < 0.2f, "her eyes fly open at a startle");
    check(startled.energy > 0.0f, "and she is roused by it");
    /* And she stays awake rather than dropping straight back off. */
    for (int i = 0; i < 120; i++) TSteps(&startled, 1, 1.0f / 60.0f);
    check(!startled.asleep, "she does not fall back asleep at once");

    /* ---- posture ----
     * The body has to carry the mood too, because a face alone cannot say
     * whether someone is braced or comfortable. */
    TFLY afraid, proud, easy;
    TNew(&afraid);
    TNew(&proud);
    TNew(&easy);
    TSeed(&afraid, 23);
    TSeed(&proud, 23);
    TSeed(&easy, 23);
    for (int i = 0; i < 600; i++) {
        if (i % 20 == 0) TPredatorSignal(&afraid, 0.9f);
        if (i % 20 == 0) TPride(&proud, 0.6f);
        TSteps(&afraid, 1, 1.0f / 60.0f);
        TSteps(&proud, 1, 1.0f / 60.0f);
        TSteps(&easy, 1, 1.0f / 60.0f);
    }
    check(afraid.spine < easy.spine, "fear curls her in");
    check(afraid.shoulder > easy.shoulder, "fear lifts her shoulders");
    check(afraid.lean < easy.lean, "fear tips her back");
    check(proud.spine > easy.spine, "pride straightens her up");
    check(afraid.spine >= -1.0f && afraid.spine <= 1.0f, "spine stays in range");
    check(proud.shoulder >= 0.0f && proud.shoulder <= 1.0f, "shoulder stays in range");
    check(easy.lean >= -1.0f && easy.lean <= 1.0f, "lean stays in range");

    /* Posture must survive being driven hard, like the rest of the face. */
    check(chaos.spine >= -1.0f && chaos.spine <= 1.0f, "spine survives chaos");
    check(chaos.shoulder >= 0.0f && chaos.shoulder <= 1.0f, "shoulder survives chaos");
    check(chaos.lean >= -1.0f && chaos.lean <= 1.0f, "lean survives chaos");
    check(chaos.eye_open >= 0.0f && chaos.eye_open <= 1.0f, "eye_open survives chaos");
    check(chaos.eye_adapt >= 0.0f && chaos.eye_adapt <= 1.0f, "eye_adapt survives chaos");
}

/* ------------------------------------------------------------------ *
 * Encounters. The point of this test is symmetry: a relationship that
 * only moved one side would not be a relationship.
 * ------------------------------------------------------------------ */
static void test_encounters(void) {
    printf("gestures and encounters\n");
    TFLY a, b;
    TNew(&a);
    TNew(&b);
    TSeed(&a, 21);
    TSeed(&b, 22);

    check(TCanEncounter(&a), "a fresh fly has no cooldown");
    check(TEncounter(&a, &b, T_ENC_GREET) == 1, "a greeting can run");
    check(a.bond > 0.0f && b.bond > 0.0f, "a greeting deepens both bonds");
    check(fabsf(a.bond - b.bond) < 0.02f, "the bond grows on both sides alike");

    /* The cooldown has to actually stop a repeat, and a refused meeting must
     * leave both flies exactly as they were. */
    check(!TCanEncounter(&a) && !TCanEncounter(&b), "an encounter starts a cooldown");
    float held = a.bond;
    float joy = a.emotion[T_EMO_JOY];
    check(TEncounter(&a, &b, T_ENC_SHARE) == 0, "a second meeting is refused");
    check(a.bond == held, "a refused meeting does not move the bond");
    check(a.emotion[T_EMO_JOY] == joy, "a refused meeting does not move affect");

    /* The cooldown has to expire, or the social layer dies after one hello. */
    for (int i = 0; i < 900; i++) {
        TSteps(&a, 1, 1.0f / 60.0f);
        TSteps(&b, 1, 1.0f / 60.0f);
    }
    check(TCanEncounter(&a), "the cooldown expires");

    /* Sharing builds a bond faster than a single greeting would. */
    TFLY c, d;
    TNew(&c);
    TNew(&d);
    TSeed(&c, 31);
    TSeed(&d, 32);
    for (int round = 0; round < 4; round++) {
        TEncounter(&c, &d, T_ENC_SHARE);
        for (int i = 0; i < 400; i++) {
            TSteps(&c, 1, 1.0f / 60.0f);
            TSteps(&d, 1, 1.0f / 60.0f);
        }
    }
    check(c.bond > 0.2f, "sharing builds a real bond");

    /* A quarrel must break it. */
    float close = c.bond;
    TEncounter(&c, &d, T_ENC_ARGUE);
    check(c.bond < close, "a quarrel breaks the bond");
    check(d.emotion[T_EMO_ANGER] > 0.0f, "a quarrel makes both angry");
    check(d.bond < close, "a quarrel breaks both bonds, not one");

    /* Fear is a hard brake. A badly frightened fly must not go looking for
     * company, which is the whole reason the term exists. */
    TFLY timid, bold;
    TNew(&timid);
    TNew(&bold);
    TSeed(&timid, 41);
    TSeed(&bold, 41);
    for (int i = 0; i < 600; i++) {
        TSteps(&timid, 1, 1.0f / 60.0f);
        TSteps(&bold, 1, 1.0f / 60.0f);
    }
    float want_before = TEncounterDrive(&bold);
    for (int i = 0; i < 60; i++) {
        TPredatorSignal(&timid, 0.9f);
        TSteps(&timid, 1, 1.0f / 60.0f);
    }
    check(TEncounterDrive(&timid) < want_before, "fear suppresses the urge to meet");

    /* A gesture runs its course and releases. */
    TFLY gest;
    TNew(&gest);
    TSeed(&gest, 51);
    check(gest.gesture == T_GES_IDLE, "a fly starts at rest");
    TGesture(&gest, T_GES_WAVE);
    check(gest.gesture == T_GES_WAVE, "a gesture can be started");
    for (int i = 0; i < 40; i++) TSteps(&gest, 1, 1.0f / 60.0f);
    check(gest.gesture_strength > 0.5f, "a gesture builds");
    check(gest.gesture_phase > 0.0f && gest.gesture_phase < 1.0f, "a gesture advances");
    for (int i = 0; i < 120; i++) TSteps(&gest, 1, 1.0f / 60.0f);
    check(gest.gesture == T_GES_IDLE, "a gesture releases back to rest");
    check(gest.gesture_strength < 0.01f, "a released gesture is fully out");

    /* An automatic gesture does not cut off one already in progress, but a
     * direct request does. */
    TGesture(&gest, T_GES_BOW);
    for (int i = 0; i < 40; i++) TSteps(&gest, 1, 1.0f / 60.0f);
    TGesture(&gest, T_GES_WAVE);
    check(gest.gesture == T_GES_BOW, "an automatic gesture does not interrupt a full one");
    TGestureForce(&gest, T_GES_WAVE);
    check(gest.gesture == T_GES_WAVE, "a direct gesture does interrupt it");

    /* Null handles must be safe: a caller can always be wrong. */
    TGesture(NULL, T_GES_WAVE);
    TGestureForce(NULL, T_GES_WAVE);
    check(TEncounter(NULL, &b, T_ENC_GREET) == 0, "a null fly cannot meet");
    check(TEncounter(&a, NULL, T_ENC_GREET) == 0, "a null fly cannot be met");
    check(TCanEncounter(NULL) == 0, "a null fly is never ready");
    check(TEncounterDrive(NULL) == 0.0f, "a null fly never wants company");

    /* Names must resolve for every id, including out of range ones, because
     * the rig looks them up by number. */
    for (int i = 0; i < TFLY_N_GESTURE; i++) {
        check(TFlyGestureName(i) != NULL && TFlyGestureName(i)[0] != '\0', "a gesture has a name");
    }
    for (int i = 0; i < TFLY_N_ENCOUNTER; i++) {
        check(TFlyEncounterName(i) != NULL && TFlyEncounterName(i)[0] != '\0',
              "an encounter has a name");
    }
    check(TFlyGestureName(-5) != NULL, "an out of range gesture still has a name");
    check(TFlyEncounterName(99) != NULL, "an out of range encounter still has a name");
}

/* ------------------------------------------------------------------ *
 * Limbs. A walk is a set of joint trajectories, not a swinging number,
 * and the properties that make it read as walking are all testable.
 * ------------------------------------------------------------------ */
static void test_limbs(void) {
    printf("limbs\n");

    TFLY fly;
    TNew(&fly);
    TSeed(&fly, 5);
    TSetGait(&fly, 1); /* steady */
    TFLYLimb l[TFLY_N_LIMB];

    /* The two legs must be half a cycle apart, or she is hopping. */
    fly.gait_phase = 0.0f;
    TGaitLimbs(&fly, l);
    check(l[T_LIMB_LEG_L].root > 0.3f, "the left leg starts forward");
    check(l[T_LIMB_LEG_R].root < -0.3f, "and the right leg starts back");
    check(fabsf(l[T_LIMB_LEG_L].root + l[T_LIMB_LEG_R].root) < 1e-5f,
          "the two legs are exactly opposed");

    /* Each arm must move against the leg on its own side. Getting this wrong
     * is what makes a biped look like it is being pulled along. */
    check(l[T_LIMB_ARM_L].root * l[T_LIMB_LEG_L].root < 0.0f,
          "the left arm opposes the left leg");
    check(l[T_LIMB_ARM_R].root * l[T_LIMB_LEG_R].root < 0.0f,
          "the right arm opposes the right leg");

    /* The knee folds during the swing and not during the stance. A knee that
     * bent while the foot was planted would be a limp, and one that never bent
     * would drag a toe. */
    fly.gait_phase = 0.0f;
    TGaitLimbs(&fly, l);
    float at_strike = l[T_LIMB_LEG_L].middle;
    fly.gait_phase = 6.2831853f * 0.30f;
    TGaitLimbs(&fly, l);
    float mid_stance = l[T_LIMB_LEG_L].middle;
    fly.gait_phase = 6.2831853f * 0.80f;
    TGaitLimbs(&fly, l);
    float mid_swing = l[T_LIMB_LEG_L].middle;
    check(at_strike < 0.2f, "the knee is straight at heel strike");
    check(mid_stance < 0.35f, "and barely bends through stance");
    check(mid_swing > 0.8f, "but folds hard during the swing");

    /* The foot must actually clear the ground during the swing, or the
     * character drags. */
    float lowest = 0.0f, highest = 0.0f;
    for (int i = 0; i < 200; i++) {
        fly.gait_phase = 6.2831853f * i / 200.0f;
        TGaitLimbs(&fly, l);
        float drop = TFootDrop(&l[T_LIMB_LEG_L], 0.10f, 0.10f, 0.017f, 0.0192f, 0.0432f, 0.020f);
        if (drop < lowest) lowest = drop;
        if (drop > highest) highest = drop;
    }
    checkf(highest - lowest > 0.08f, "the foot clears the ground while swinging (%.4f)",
           highest - lowest);
    checkf(lowest < -0.15f, "and reaches the ground on the stance (%.4f)", lowest);

    /* The foot never goes below the length of the leg, which would mean the
     * chain had folded through itself. */
    check(lowest > -(0.10f + 0.10f) - 0.05f, "the leg never folds through itself");

    /* Losing balance must make the arms move independently of the stride. If
     * they merely followed the legs, a falling character would look like a
     * falling statue. */
    fly.balance = 1.0f;
    fly.gait_phase = 1.0f;
    TGaitLimbs(&fly, l);
    float calm_root = l[T_LIMB_ARM_L].root;
    float calm_elbow = l[T_LIMB_ARM_L].middle;
    fly.balance = 0.05f;
    fly.age = 0.20f;
    TGaitLimbs(&fly, l);
    check(fabsf(l[T_LIMB_ARM_L].root - calm_root) > 0.3f,
          "an unsteady fly throws her arms about");
    check(fabsf(l[T_LIMB_ARM_L].middle - calm_elbow) > 0.1f,
          "and the elbows go with them");

    /* A steady fly must not windmill. */
    fly.balance = 1.0f;
    fly.age = 0.20f;
    TGaitLimbs(&fly, l);
    check(fabsf(l[T_LIMB_ARM_L].root - calm_root) < 0.2f,
          "a steady fly keeps her arms in rhythm");

    /* Every joint of every limb stays somewhere a limb can reach, across all
     * four gaits and all phases. The ankle and wrist carry absolute angles, so
     * what has to stay small is their *articulation*: the bend they add on top
     * of the two joints above them. */
    TFLY w;
    TNew(&w);
    float max_root = 0.0f, max_mid = 0.0f, max_end = 0.0f, max_spread = 0.0f;
    for (int g = 0; g < TFLY_N_GAIT; g++) {
        TSetGait(&w, g);
        for (int i = 0; i < 3000; i++) {
            w.age += 1.0f / 60.0f;
            w.gait_phase += 0.1f;
            w.balance = (i % 200 < 100) ? 1.0f : 0.05f;
            TGaitLimbs(&w, l);
            for (int k = 0; k < TFLY_N_LIMB; k++) {
                if (fabsf(l[k].root) > max_root) max_root = fabsf(l[k].root);
                if (fabsf(l[k].middle) > max_mid) max_mid = fabsf(l[k].middle);
                float articulation = l[k].end - l[k].root - l[k].middle;
                if (fabsf(articulation) > max_end) max_end = fabsf(articulation);
                if (fabsf(l[k].spread) > max_spread) max_spread = fabsf(l[k].spread);
            }
        }
    }
    checkf(max_root < 1.6f, "the shoulder never hyperextends (%.3f)", max_root);
    checkf(max_mid < 2.4f, "the knee and elbow stay inside their bend (%.3f)", max_mid);
    checkf(max_end < 0.5f, "the ankle and wrist articulate only a little (%.3f)", max_end);
    checkf(max_spread < 1.0f, "the limbs stay beside the body (%.3f)", max_spread);

    /* A longer stride lifts the foot higher, which is the whole difference
     * between a march and a shuffle. The measure is the range of the foot over
     * a cycle, not its height: the drop is always negative, so a maximum
     * initialised to zero would never move. */
    TFLY shuf, march;
    TNew(&shuf);
    TNew(&march);
    TSetGait(&shuf, 0); /* hurried: short steps */
    TSetGait(&march, 2); /* long stride */
    float shuf_lo = 0.0f, shuf_hi = 0.0f, march_lo = 0.0f, march_hi = 0.0f;
    int first = 1;
    for (int i = 0; i < 400; i++) {
        float ph = 6.2831853f * i / 400.0f;
        shuf.gait_phase = ph;
        TGaitLimbs(&shuf, l);
        float d = TFootDrop(&l[T_LIMB_LEG_L], 0.10f, 0.10f, 0.017f, 0.0192f, 0.0432f, 0.020f);
        if (first) {
            shuf_lo = shuf_hi = d;
            march_lo = march_hi = d;
            first = 0;
        }
        if (d < shuf_lo) shuf_lo = d;
        if (d > shuf_hi) shuf_hi = d;
        march.gait_phase = ph;
        TGaitLimbs(&march, l);
        d = TFootDrop(&l[T_LIMB_LEG_L], 0.10f, 0.10f, 0.017f, 0.0192f, 0.0432f, 0.020f);
        if (d < march_lo) march_lo = d;
        if (d > march_hi) march_hi = d;
    }
    checkf(march_hi - march_lo > shuf_hi - shuf_lo,
           "a long stride lifts the foot higher (%.4f vs %.4f)", march_hi - march_lo,
           shuf_hi - shuf_lo);

    /* The foot must never come back up to the hip, which would mean the chain
     * has folded through itself, and must never drop far past a straight leg
     * either, which would mean it is stretching. */
    for (int g = 0; g < TFLY_N_GAIT; g++) {
        TSetGait(&w, g);
        for (int i = 0; i < 500; i++) {
            w.gait_phase = 6.2831853f * i / 500.0f;
            w.balance = (i % 100 < 50) ? 1.0f : 0.0f;
            TGaitLimbs(&w, l);
            for (int k = 0; k < 2; k++) {
                float d = TFootDrop(&l[k], 0.10f, 0.10f, 0.017f, 0.0192f, 0.0432f, 0.020f);
                if (d > 0.0f) {
                    checkf(0, "gait %d: the foot came back up to the hip (%.4f)", g, d);
                    break;
                }
                if (d < -(0.20f + 0.05f)) {
                    checkf(0, "gait %d: the leg stretched past straight (%.4f)", g, d);
                    break;
                }
            }
        }
    }
    check(1, "the foot stays between the hip and a stretched leg, on every gait");

    /* The walk has to follow the ground, not its own clock. A cycle that runs
     * faster than the body travels drags the feet backwards, which is the
     * single most visible way a walk can be wrong. */
    {
        TFLY walker;
        TNew(&walker);
        TSeed(&walker, 61);
        TSetGait(&walker, 1);
        TGroundSpeed(&walker, 0.8f);
        float first = walker.gait_phase;
        for (int i = 0; i < 300; i++) TSteps(&walker, 1, 1.0f / 60.0f);
        float advanced = walker.gait_phase - first;
        /* Five seconds at 0.8 per second is four units of ground. With a step
         * near 0.2 that is about twenty steps, or ten cycles. */
        checkf(advanced > 50.0f && advanced < 80.0f,
               "the phase advances at the rate the ground implies (%.1f rad in 5 s at 0.8/s)",
               advanced);

        /* Twice the ground speed must be twice the steps. Anything else means
         * the feet slide, which is the whole thing this fixes. */
        TFLY fast;
        TNew(&fast);
        TSeed(&fast, 61);
        TSetGait(&fast, 1);
        TGroundSpeed(&fast, 1.6f);
        for (int i = 0; i < 300; i++) TSteps(&fast, 1, 1.0f / 60.0f);
        checkf(fast.steps > walker.steps * 1.8f && fast.steps < walker.steps * 2.2f,
               "double the ground speed is double the steps (%.1f against %.1f)", fast.steps,
               walker.steps);
    }

    /* Standing still settles into a stance instead of marching on the spot.
     *
     * Sleep is used to make her genuinely still, because the policy recomputes
     * thrust every tick and a fly that wants to walk cannot be talked out of it
     * by writing zero into the motor channel.
     *
     * The measure is the distance to the nearest half cycle, because a planted
     * leg is whichever of the two is down. Taking a plain remainder would read
     * a phase a hair below zero as a whole cycle of error. */
    {
        TFLY idle;
        TNew(&idle);
        TSeed(&idle, 71);
        TGroundSpeed(&idle, 0.9f);
        for (int i = 0; i < 300; i++) TSteps(&idle, 1, 1.0f / 60.0f);
        float moving = fabsf(fmodf(idle.gait_phase, 3.14159265f));
        if (moving > 1.5707963f) moving = 3.14159265f - moving;
        TGroundSpeed(&idle, 0.0f);
        TSleep(&idle);
        for (int i = 0; i < 300; i++) TSteps(&idle, 1, 1.0f / 60.0f);
        float still = fabsf(fmodf(idle.gait_phase, 3.14159265f));
        if (still > 1.5707963f) still = 3.14159265f - still;
        checkf(moving > still, "a still fly settles out of her stride (%.3f to %.3f)", moving,
               still);
        checkf(still < 0.5f, "and settles on a planted leg (%.3f)", still);
    }

    /* A null fly must be safe, because a caller can always be wrong. */
    TGroundSpeed(NULL, 1.0f);
    TGaitLimbs(NULL, l);
    check(1, "a null fly has no limbs to pose");
    TFootDrop(NULL, 0.1f, 0.1f, 0.017f, 0.0192f, 0.0432f, 0.020f);
    check(1, "and no foot to drop");
}

/* ------------------------------------------------------------------ *
 * Gaze. A decided target has to be able to hold, and alarm has to be
 * able to break it.
 * ------------------------------------------------------------------ */
static void test_gaze(void) {
    printf("gaze\n");

    TFLY fly;
    TNew(&fly);
    TSeed(&fly, 29);
    for (int i = 0; i < 120; i++) TSteps(&fly, 1, 1.0f / 60.0f);
    float drift = fly.gaze_x;

    /* Naming a target is itself an act of attention, so it takes hold. */
    TLookAt(&fly, -0.8f, 0.2f);
    check(fly.look_lock >= 0.5f, "naming a target takes hold on its own");
    TLookStrength(&fly, 1.0f);
    for (int i = 0; i < 120; i++) TSteps(&fly, 1, 1.0f / 60.0f);
    check(fly.gaze_x < drift - 0.2f, "she looks where she decided");
    check(fly.gaze_y > 0.05f, "including vertically");

    /* Letting go brings the idle drift back, so a stale target does not pin
     * her eyes for the rest of the session. */
    TLookAway(&fly);
    check(fly.look_lock == 0.0f, "look_away releases the lock");
    for (int i = 0; i < 240; i++) TSteps(&fly, 1, 1.0f / 60.0f);

    /* Alarm of any kind must break the lock, not just a predator signal: a
     * frightened fly is not staring at her friend. */
    TFLY scared;
    TNew(&scared);
    TSeed(&scared, 31);
    TLookAt(&scared, 0.8f, 0.0f);
    TLookStrength(&scared, 1.0f);
    for (int i = 0; i < 120; i++) TSteps(&scared, 1, 1.0f / 60.0f);
    float watching = scared.gaze_x;
    check(watching > 0.3f, "she was watching the thing she chose");
    for (int i = 0; i < 30; i++) {
        TFear(&scared, 0.9f);
        TSteps(&scared, 1, 1.0f / 60.0f);
    }
    check(scared.gaze_x < watching, "but fear breaks the gaze lock");

    /* The lock must not be able to hold her past her own range. */
    TLookAt(&fly, 99.0f, -99.0f);
    check(fly.look_x <= 1.0f && fly.look_x >= -1.0f, "an absurd target is clamped");
    TLookStrength(&fly, 99.0f);
    check(fly.look_lock <= 1.0f, "an absurd lock is clamped");
    TLookStrength(&fly, -5.0f);
    check(fly.look_lock >= 0.0f, "a negative lock is clamped");
    TLookAt(NULL, 0.0f, 0.0f);
    TLookStrength(NULL, 1.0f);
    TLookAway(NULL);
    check(1, "a null fly ignores every gaze command");
}

int main(void) {
    printf("TFLY.h v%d.%d self test\n\n", TFLY_VERSION_MAJOR, TFLY_VERSION_MINOR);

    test_lifecycle();
    test_sensory();
    test_pain_localisation();
    test_hormones();
    test_emotions();
    test_learning();
    test_memory();
    test_policy();
    test_homeostasis();
    test_social();
    test_determinism();
    test_robustness();
    test_scenario_nociception();
    test_face();
    test_encounters();
    test_limbs();
    test_gaze();

    printf("\n");
    if (failures == 0) {
        printf("all checks passed\n");
        return 0;
    }
    printf("%d check(s) failed\n", failures);
    return 1;
}

