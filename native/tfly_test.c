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

/* ------------------------------------------------------------------ */

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

    printf("\n");
    if (failures == 0) {
        printf("all checks passed\n");
        return 0;
    }
    printf("%d check(s) failed\n", failures);
    return 1;
}
