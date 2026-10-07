/* C interface to the Klatter speech synthesizer. */
#ifndef KLATTER_H
#define KLATTER_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Settings in physical units. */
typedef struct {
    double wpm;        /* words per minute, > 0 */
    double pitch_hz;   /* pitch baseline; the whole contour moves with it, > 0 */
    double inflection; /* pitch-range multiplier: 0 monotone, 1 natural, >= 0 */
    double volume;     /* linear amplitude multiplier: 1 full level, >= 0 */
} KlatterVoice;

/* 16-bit mono samples, and each mark with the index of the sample it precedes. */
typedef struct {
    const int16_t *samples;
    size_t sample_count;
    const uint32_t *mark_ids;
    const size_t *mark_samples;
    size_t mark_count;
} KlatterSpeech;

void klatter_default_voice(KlatterVoice *out);

/* Speaks count items as one utterance. Item i is the UTF-8 text texts[i], or
   the mark marks[i] when texts[i] is NULL. Returns NULL for invalid input;
   release a result with klatter_free. */
KlatterSpeech *klatter_speak(const char *const *texts, const uint32_t *marks, size_t count,
                             uint32_t sample_rate, const KlatterVoice *voice);

void klatter_free(KlatterSpeech *speech);

#ifdef __cplusplus
}
#endif

#endif
