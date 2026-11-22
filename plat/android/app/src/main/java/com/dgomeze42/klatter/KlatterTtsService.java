package com.dgomeze42.klatter;

import android.annotation.TargetApi;
import android.media.AudioFormat;
import android.os.Build;
import android.speech.tts.SynthesisCallback;
import android.speech.tts.SynthesisRequest;
import android.speech.tts.TextToSpeech;
import android.speech.tts.TextToSpeechService;
import android.speech.tts.Voice;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;

/**
 * Klatter as an Android text-to-speech engine, for TalkBack, other screen readers
 * and every app that speaks, on Android 4.0 and newer.
 *
 * Android calls onSynthesizeText on one thread, one request at a time; the engine
 * is kept between requests, and audio is handed over as it is made, so speech
 * starts at once. onStop comes on another thread and takes effect at the next
 * chunk. Nothing runs between requests.
 */
@TargetApi(14)
public class KlatterTtsService extends TextToSpeechService {
    private static final String LANGUAGE = "eng";
    private static final String COUNTRY = "USA";
    /** What Android before 4.3 takes at once, which has no way to ask. */
    private static final int OLD_CHUNK_BYTES = 4096;

    private Synthesizer synthesizer;
    private volatile boolean stopRequested;

    @Override
    public void onCreate() {
        super.onCreate();
        synthesizer = new Synthesizer(this);
    }

    @Override
    public void onDestroy() {
        if (synthesizer != null) synthesizer.release();
        super.onDestroy();
    }

    // ---- Languages: American English, and English generally --------------------

    private static boolean isEnglish(String lang) {
        return lang != null && (lang.equalsIgnoreCase("eng") || lang.equalsIgnoreCase("en"));
    }

    private static boolean isUs(String country) {
        return country != null && (country.equalsIgnoreCase("USA") || country.equalsIgnoreCase("US"));
    }

    @Override
    protected int onIsLanguageAvailable(String lang, String country, String variant) {
        if (!isEnglish(lang)) return TextToSpeech.LANG_NOT_SUPPORTED;
        return isUs(country) ? TextToSpeech.LANG_COUNTRY_AVAILABLE : TextToSpeech.LANG_AVAILABLE;
    }

    @Override
    protected String[] onGetLanguage() {
        return new String[]{LANGUAGE, COUNTRY, ""};
    }

    @Override
    protected int onLoadLanguage(String lang, String country, String variant) {
        return onIsLanguageAvailable(lang, country, variant);
    }

    // ---- Voices (Android 5.0 and newer) -------------------------------------------

    /** Klatter's one voice, by a name that never changes. */
    static final String VOICE_NAME = "klatter";

    @TargetApi(21)
    @Override
    public List<Voice> onGetVoices() {
        List<Voice> voices = new ArrayList<Voice>();
        voices.add(new Voice(VOICE_NAME, Locale.US, Voice.QUALITY_HIGH, Voice.LATENCY_VERY_LOW, false,
                new HashSet<String>()));
        return voices;
    }

    @TargetApi(21)
    @Override
    public int onIsValidVoiceName(String voiceName) {
        return VOICE_NAME.equals(voiceName) ? TextToSpeech.SUCCESS : TextToSpeech.ERROR;
    }

    @TargetApi(21)
    @Override
    public int onLoadVoice(String voiceName) {
        return onIsValidVoiceName(voiceName);
    }

    @TargetApi(21)
    @Override
    public String onGetDefaultVoiceNameFor(String lang, String country, String variant) {
        return isEnglish(lang) ? VOICE_NAME : null;
    }

    // ---- Speaking ----------------------------------------------------------------------

    @Override
    protected void onStop() {
        stopRequested = true;
    }

    @Override
    protected void onSynthesizeText(SynthesisRequest request, final SynthesisCallback callback) {
        stopRequested = false;
        // getCharSequenceText is Android 5.0; before, getText (a String).
        @SuppressWarnings("deprecation")
        CharSequence text = Build.VERSION.SDK_INT >= 21 ? request.getCharSequenceText() : request.getText();
        if (text == null) text = "";
        if (!isEnglish(request.getLanguage()) && request.getLanguage() != null && request.getLanguage().length() > 0) {
            callback.error();
            return;
        }
        int chunk = Build.VERSION.SDK_INT >= 18 ? callback.getMaxBufferSize() : OLD_CHUNK_BYTES;
        chunk = Math.max(256, Math.min(chunk, 8192));
        if (callback.start(synthesizer.sampleRate(), AudioFormat.ENCODING_PCM_16BIT, 1) != TextToSpeech.SUCCESS) return;

        int result = synthesizer.speak(text, request.getSpeechRate() / 100.0, request.getPitch() / 100.0,
                chunk, new KlatterNative.AudioSink() {
                    @Override
                    public int onAudio(byte[] pcm, int length) {
                        if (stopRequested) return 1;
                        return callback.audioAvailable(pcm, 0, length) == TextToSpeech.SUCCESS ? 0 : 1;
                    }
                });
        if (result < 0 && !stopRequested) {
            callback.error();
        } else {
            callback.done();
        }
    }
}
