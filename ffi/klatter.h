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

/* Called synchronously with borrowed blocks of at most 100 ms. Mark offsets
   are relative to this block, and can equal sample_count. Empty blocks are
   cancellation checkpoints before and after utterance planning.
   Return 0 to cancel, nonzero to continue. Do not free a block or retain its
   pointers after returning. No further callbacks occur after cancellation. */
typedef int (*KlatterSpeechCallback)(const KlatterSpeech *block, void *context);

/* Returns 1 on completion, 0 on cancellation, -1 on invalid input or failure.
   Preserves utterance phrasing and synthesis state across audio blocks. */
int klatter_speak_stream(const char *const *texts, const uint32_t *marks, size_t count,
                        uint32_t sample_rate, const KlatterVoice *voice,
                        KlatterSpeechCallback callback, void *context);

void klatter_free(KlatterSpeech *speech);

typedef struct KlatterPronunciations KlatterPronunciations;
typedef struct KlatterUtterance KlatterUtterance;
typedef struct KlatterCancel KlatterCancel;

KlatterPronunciations *klatter_pronunciations_create(void);
/* One English word and ARPAbet phones; vowels require stress 0, 1 or 2.
   Returns 1 on success, -1 on invalid input/failure, leaving any old entry intact.
   Do not mutate/free a dictionary concurrently with another access. */
int klatter_pronunciations_set(KlatterPronunciations *dictionary,
                              const char *word, const char *phones);
void klatter_pronunciations_free(KlatterPronunciations *dictionary);

typedef struct {
    size_t input_item;
    size_t byte_offset;
    size_t byte_length;
    size_t sample; /* absolute synthesized sample index, not device playback time */
} KlatterWord;

typedef struct {
    size_t sample_count;
    const uint32_t *mark_ids;
    const size_t *mark_samples; /* absolute sample positions */
    size_t mark_count;
    const KlatterWord *words;
    size_t word_count;
} KlatterPullBlock;

/* Copies input text and an optional override dictionary; null selects defaults.
   Returns NULL on invalid input/failure. Does no acoustic planning until read.
   Plans the whole utterance to preserve its prosody.
   A handle must not be read/freed concurrently. */
KlatterUtterance *klatter_utterance_create(
    const char *const *texts, const uint32_t *marks, size_t count,
    uint32_t sample_rate, const KlatterVoice *voice,
    const KlatterPronunciations *dictionary);

/* Fills caller-owned PCM, zeroing unused samples. Returns 1 for output/events,
   0 for EOF, -2 for cancellation, -1 for invalid input/failure. At capacity zero,
   no work is done and 1 means still active. Metadata is borrowed until the next
   read/free; expanded words may share original UTF-8 byte ranges. Inspect events
   even when sample_count is zero. Already queued device audio remains the host's
   responsibility. Planning polls cancellation between source tokens, segments
   and pitch frames; individual lookups and allocations are not preemptible. */
int klatter_utterance_read(KlatterUtterance *utterance, int16_t *output,
                          size_t capacity, KlatterPullBlock *block);
void klatter_utterance_free(KlatterUtterance *utterance);

/* Obtain before starting concurrent reads. The independent cancel handle may be
   used from another thread and outlive its utterance. Cancellation is permanent
   for that utterance only. Do not free it concurrently with cancellation. */
KlatterCancel *klatter_utterance_cancel_handle(const KlatterUtterance *utterance);
void klatter_cancel(const KlatterCancel *cancel);
void klatter_cancel_free(KlatterCancel *cancel);

#ifdef __cplusplus
}
#endif

#endif
