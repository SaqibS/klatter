package com.dgomeze42.klatter;

import android.content.Context;

/**
 * Speaks with Klatter as the settings say: the speech service, the settings
 * screen's Speak button and its WAV export all use this, so all three sound alike.
 * The dictionaries are loaded once and kept until they change. Use one
 * Synthesizer from one thread at a time.
 */
final class Synthesizer {
    private final Context context;
    private final Settings settings;
    private long dictionary;
    private int dictionaryVersion = -1;

    Synthesizer(Context context) {
        this.context = context.getApplicationContext();
        this.settings = new Settings(this.context);
    }

    Settings settings() {
        return settings;
    }

    /** The rate the next utterance will come out at. */
    int sampleRate() {
        return settings.sampleRate();
    }

    /**
     * Speaks text, handing PCM to the sink in chunks of at most chunkBytes.
     *
     * @param rateScale  the screen reader's rate, 1 being normal (Android's 100)
     * @param pitchScale the screen reader's pitch, 1 being normal
     * @return KlatterNative.OK, STOPPED, or a negative error
     */
    int speak(CharSequence text, double rateScale, double pitchScale, int chunkBytes, KlatterNative.AudioSink sink) {
        int version = settings.dictionaryVersion();
        if (version != dictionaryVersion) {
            if (dictionary != 0) KlatterNative.dictionaryFree(dictionary);
            dictionary = Dictionaries.build(context, settings);
            dictionaryVersion = version;
        }
        double scale = settings.followSystemRate() ? rateScale : 1.0;
        double wpm = Math.max(Settings.MIN_WPM, Math.min(Settings.MAX_WPM, settings.rateWpm() * scale));
        double pitch = settings.exactPitch() ? settings.pitchHz() : Settings.NORMAL_PITCH;
        if (settings.followSystemPitch()) pitch *= pitchScale;
        pitch = Math.max(Settings.MIN_PITCH, Math.min(Settings.MAX_PITCH, pitch));
        return KlatterNative.speak(Parameters.utf8(text.toString()), wpm, pitch, settings.inflection() / 100.0,
                settings.volume() / 100.0, settings.sampleRate(), Parameters.array(settings.quality()), dictionary,
                chunkBytes, sink);
    }

    void release() {
        if (dictionary != 0) KlatterNative.dictionaryFree(dictionary);
        dictionary = 0;
        dictionaryVersion = -1;
    }
}
