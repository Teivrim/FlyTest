/*
 * tfly_demo.c - a scripted scene that exercises the TFLY API and prints the
 * fly's internal state as readable text.
 *
 * Build:
 *   gcc -std=c99 -O2 -o tfly_demo native/tfly_demo.c -lm
 *
 * The scene is deliberately legible: a fly is born, finds food, learns that
 * a colour predicts it, is injured, refuses to court while afraid, and is
 * eventually talked out of it. Each phase prints the state that changed.
 */

#include <stdio.h>
#include <string.h>

#include "../TFLY.h"

static void header(const char *title) {
    printf("\n=== %s ===\n", title);
}

static void report(const TFLY *fly, const char *note) {
    printf("[%7.2fs] %-28s mood=%-10s drive=%-9s v=%+.2f a=%.2f thrust=%.2f turn=%+.2f beat=%.0fHz\n",
           (double)fly->t, note, TFlyEmotionName(TDominantEmotion((TFLY *)fly)),
           TFlyDriveName(TDominantDrive((TFLY *)fly)), (double)fly->valence, (double)fly->arousal,
           (double)fly->out_thrust, (double)fly->out_turn, (double)fly->out_wingbeat);
}

int main(void) {
    TFLY fly;
    TNew(&fly);
    TSeed(&fly, 20260925);
    printf("TFLY demo, version %d.%d\n", TFLY_VERSION_MAJOR, TFLY_VERSION_MINOR);

    /* ---- phase 1: a fly wakes up in an empty, dark arena ---- */
    header("phase 1: newborn, dark arena");
    TLight(&fly, 0.05f);
    TSteps(&fly, 300, 1.0f / 60.0f);
    report(&fly, "fresh");

    /* ---- phase 2: the light comes on ---- */
    header("phase 2: light on");
    TLight(&fly, 0.7f);
    TSteps(&fly, 300, 1.0f / 60.0f);
    report(&fly, "lights on");
    printf("  learning gate is now %.3f (dopamine=%.3f octopamine=%.3f)\n",
           (double)TLearningGate(&fly), (double)fly.hormone[T_H_DOPAMINE],
           (double)fly.hormone[T_H_OCTOPAMINE]);

    /* ---- phase 3: food appears, and a positive association is formed ----
     * This is the three-factor rule in action. The weight only moves when a
     * neural trace of the action is present, which TPolicy maintains, and a
     * modulator is available, which TReward provides by releasing dopamine.
     * Remove either factor and nothing is learned. */
    header("phase 3: fruit odour plus sugar");
    printf("  start weight fruit->forward = %+.4f, gate = %.3f\n",
           (double)fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD], (double)TLearningGate(&fly));
    for (int i = 0; i < 40; i++) {
        TOdor(&fly, 0.6f, T_ODOR_FRUIT);
        TSteps(&fly, 20, 1.0f / 60.0f); /* let the fly act, building eligibility */
        TTaste(&fly, 0.8f, 0.0f, 0.0f, 0.0f);
        TReward(&fly, 0.6f);
        TAssociate(&fly, T_CUE_ODOR_FRUIT, T_ACT_FORWARD, 1.0f, TLearningGate(&fly));
        TSteps(&fly, 20, 1.0f / 60.0f);
    }
    report(&fly, "fed");
    printf("  learned weight fruit->forward = %+.4f\n",
           (double)fly.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD]);
    printf("  hunger is now %.3f, energy %.3f\n", (double)fly.drive[T_DRIVE_HUNGER],
           (double)fly.energy);

    /* control: the same trials with no modulator must not learn anything */
    {
        TFLY control;
        TNew(&control);
        TSeed(&control, 20260925);
        for (int i = 0; i < 40; i++) {
            TOdor(&control, 0.6f, T_ODOR_FRUIT);
            TSteps(&control, 20, 1.0f / 60.0f);
            TAssociate(&control, T_CUE_ODOR_FRUIT, T_ACT_FORWARD, 1.0f, 0.0f);
            TSteps(&control, 20, 1.0f / 60.0f);
        }
        printf("  control with gate=0:        %+.4f  (must stay 0.0000)\n",
               (double)control.assoc[T_CUE_ODOR_FRUIT][T_ACT_FORWARD]);
    }

    /* ---- phase 4: a shock teaches avoidance ---- */
    header("phase 4: punishment on the light cue");
    float w_before = fly.assoc[T_CUE_LIGHT][T_ACT_FORWARD];
    for (int i = 0; i < 20; i++) {
        TLight(&fly, 0.7f);
        TSteps(&fly, 20, 1.0f / 60.0f);
        THeart(&fly, 0.7f, T_BODY_WING_L);
        TAssociate(&fly, T_CUE_LIGHT, T_ACT_FORWARD, -1.0f, TLearningGate(&fly));
        TSteps(&fly, 20, 1.0f / 60.0f);
    }
    report(&fly, "shocked");
    printf("  light->forward moved from %+.4f to %+.4f\n", (double)w_before,
           (double)fly.assoc[T_CUE_LIGHT][T_ACT_FORWARD]);
    printf("  pain total %.3f, stress %.3f, fear %.3f\n", (double)fly.pain_total,
           (double)fly.stress, (double)fly.emotion[T_EMO_FEAR]);

    /* ---- phase 5: a mate arrives, but fear wins ---- */
    header("phase 5: courtship blocked by fear");
    TMateSignal(&fly, 1.0f);
    TSteps(&fly, 60, 1.0f / 60.0f);
    report(&fly, "mate present, still hurt");
    printf("  mate action = %.3f, lust = %.3f\n", (double)fly.action[T_ACT_MATE],
           (double)fly.emotion[T_EMO_LUST]);

    /* ---- phase 6: recovery, then courtship ---- */
    header("phase 6: recovery");
    THealAll(&fly);
    TContentment(&fly, 0.6f);
    TJoy(&fly, 0.4f);
    TSteps(&fly, 600, 1.0f / 60.0f);
    report(&fly, "recovered");
    printf("  mate action = %.3f\n", (double)fly.action[T_ACT_MATE]);

    /* ---- phase 7: forced sleep and wake ---- */
    header("phase 7: sleep cycle");
    TSleep(&fly);
    TSteps(&fly, 30, 1.0f / 60.0f);
    printf("  asleep=%d wingbeat=%.1f thrust=%.2f\n", fly.asleep, (double)fly.out_wingbeat,
           (double)fly.out_thrust);
    int woke = 0;
    for (int i = 0; i < 3600 && !woke; i++) {
        TSteps(&fly, 1, 1.0f / 60.0f);
        if (fly.asleep == 0) woke = 1;
    }
    printf("  woke after the sleep drive fell: asleep=%d energy=%.3f\n", fly.asleep,
           (double)fly.energy);

    /* ---- final report ---- */
    header("final state");
    TPrintState(&fly);
    TPrintEmotions(&fly);
    TPrintHormones(&fly);

    {
        char buf[512];
        TToJson(&fly, buf, (int)sizeof(buf));
        printf("\njson: %s\n", buf);
    }

    printf("\ndemo complete\n");
    return 0;
}
