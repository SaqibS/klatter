package com.dgomeze42.klatter;

import android.annotation.TargetApi;
import android.content.Context;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;
import android.speech.tts.TextToSpeech;
import android.speech.tts.UtteranceProgressListener;
import android.util.Log;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * A check of Klatter through Android's own text-to-speech service, for testing
 * without a person listening: speech is saved to files through the service at
 * normal and double speed, with flutter off and on, the files are compared, the
 * sampling rate is checked, and how soon the first audio arrives is timed. The settings are
 * put back afterwards. Results go to the log under "KlatterSelfTest", the files to
 * the app's external files folder. Started with
 *   adb shell am start -a com.dgomeze42.klatter.SELF_TEST -n com.dgomeze42.klatter/.SettingsActivity
 */
@TargetApi(21)
final class SelfTest {
    static final String TAG = "KlatterSelfTest";
    private static final String TEXT = "Hello from Klatter on Android. The quick brown fox jumps over the lazy dog.";

    private SelfTest() {}

    static void run(final Context context) {
        if (Build.VERSION.SDK_INT < 21) {
            Log.i(TAG, "needs Android 5.0 or newer");
            return;
        }
        Log.i(TAG, Parameters.ALL.size() + " voice-quality settings");
        long d = KlatterNative.dictionaryNew();
        int[] counts = KlatterNative.dictionaryLoad(d, "# test\nklatter K L AE1 T ER0\nbad B QQ1 D\n".getBytes(Parameters.UTF8));
        KlatterNative.dictionaryFree(d);
        boolean ok = counts[0] == 1 && counts[1] == 1 && counts[2] == 3;
        Log.i(TAG, (ok ? "OK" : "FAIL") + " dictionary: " + counts[0] + " taken, " + counts[1] + " left out at line " + counts[2]);

        checkEverySetting(context);

        final TextToSpeech[] holder = new TextToSpeech[1];
        holder[0] = new TextToSpeech(context, new TextToSpeech.OnInitListener() {
            public void onInit(int status) {
                if (status != TextToSpeech.SUCCESS) {
                    Log.e(TAG, "FAIL: the speech service did not start (" + status + ")");
                    return;
                }
                new Run(context, holder[0]).next();
            }
        }, context.getPackageName());
    }

    /**
     * Every voice-quality setting, at the lowest and the highest it goes, must change
     * the sound, and every sampling rate must give audio at that rate. Spoken
     * directly through the Synthesizer, as the speech service speaks; the user's
     * settings are put back afterwards.
     */
    private static void checkEverySetting(Context context) {
        Synthesizer synthesizer = new Synthesizer(context);
        Settings settings = synthesizer.settings();
        java.util.Map<String, Double> saved = settings.quality();
        int savedRate = settings.sampleRate();
        settings.resetQuality();
        settings.setSampleRate(Settings.DEFAULT_SAMPLE_RATE);
        String plain = render(synthesizer);
        int ok = 0, bad = 0;
        for (Parameters.Parameter p : Parameters.ALL) {
            for (double v : new double[]{p.min, p.max}) {
                if (v == p.defaultValue) continue;
                settings.setQuality(p, v);
                boolean changed = !render(synthesizer).equals(plain);
                settings.setQuality(p, null);
                if (changed) ok++;
                else {
                    bad++;
                    Log.i(TAG, "FAIL " + p.name + " at " + Parameters.shown(v) + " changed nothing");
                }
            }
        }
        Log.i(TAG, (bad == 0 ? "OK" : "FAIL") + " voice quality: " + ok + " of " + (ok + bad)
                + " settings at either end change the sound");
        int rates = 0;
        for (int rate : Settings.sampleRates()) {
            settings.setSampleRate(rate);
            final int[] bytes = {0};
            synthesizer.speak("Hello.", 1, 1, 4096, new KlatterNative.AudioSink() {
                public int onAudio(byte[] pcm, int length) { bytes[0] += length; return 0; }
            });
            // "Hello." is about half a second: at least a third of a second of audio.
            if (bytes[0] > rate * 2 / 3) rates++;
            else Log.i(TAG, "FAIL sampling rate " + rate + ": " + bytes[0] + " bytes");
        }
        Log.i(TAG, (rates == Settings.sampleRates().length ? "OK" : "FAIL") + " sampling rates: "
                + rates + " of " + Settings.sampleRates().length + " give audio at their rate");
        settings.resetQuality();
        for (java.util.Map.Entry<String, Double> e : saved.entrySet()) {
            settings.setQuality(Parameters.named(e.getKey()), e.getValue());
        }
        settings.setSampleRate(savedRate);
        synthesizer.release();
    }

    private static String render(Synthesizer synthesizer) {
        try {
            final MessageDigest md = MessageDigest.getInstance("SHA-1");
            synthesizer.speak("She sells fish, and he hopes to sing a happy tune.", 1, 1, 4096,
                    new KlatterNative.AudioSink() {
                        public int onAudio(byte[] pcm, int length) { md.update(pcm, 0, length); return 0; }
                    });
            return android.util.Base64.encodeToString(md.digest(), android.util.Base64.NO_WRAP);
        } catch (Exception e) {
            return "";
        }
    }

    /** One recording: flutter, a speed and a sampling rate. */
    private static final class Job {
        final int flutter;
        final float speed;
        final int sampleRate;

        Job(int flutter, float speed, int sampleRate) {
            this.flutter = flutter;
            this.speed = speed;
            this.sampleRate = sampleRate;
        }

        String name() {
            return "flutter" + flutter + "-x" + (int) speed + "-" + sampleRate;
        }
    }

    private static final class Run extends UtteranceProgressListener {
        private final Context context;
        private final TextToSpeech tts;
        private final Settings settings;
        private final int savedRate;
        private final Double savedFlutter;
        private final Parameters.Parameter flutter = Parameters.named("fl");
        private final List<Job> jobs = new ArrayList<Job>();
        private final Set<String> normalSpeedDigests = new HashSet<String>();
        private int normalSpeedFiles;
        private long started, firstAudio;
        private int index = -1;

        Run(Context context, TextToSpeech tts) {
            this.context = context;
            this.tts = tts;
            this.settings = new Settings(context);
            savedRate = settings.sampleRate();
            savedFlutter = settings.quality().get("fl");
            tts.setOnUtteranceProgressListener(this);
            for (int f : new int[]{0, 25}) {
                jobs.add(new Job(f, 1.0f, Settings.DEFAULT_SAMPLE_RATE));
                jobs.add(new Job(f, 2.0f, Settings.DEFAULT_SAMPLE_RATE));
            }
            // The lowest rate, 48 kHz, and the rates above it, through the service.
            jobs.add(new Job(0, 1.0f, 8000));
            for (int rate : Settings.sampleRates()) if (rate >= 48000) jobs.add(new Job(0, 1.0f, rate));
            Log.i(TAG, "sampling rates offered: " + Settings.sampleRates().length + ", highest " + Settings.highestSampleRate());
        }

        private File file(Job j) {
            return new File(context.getExternalFilesDir(null), j.name() + ".wav");
        }

        void next() {
            index++;
            if (index >= jobs.size()) {
                settings.setQuality(flutter, savedFlutter);
                settings.setSampleRate(savedRate);
                Log.i(TAG, (normalSpeedDigests.size() == normalSpeedFiles ? "OK" : "FAIL") + " flutter: "
                        + normalSpeedDigests.size() + " different recordings from " + normalSpeedFiles + " settings");
                Log.i(TAG, "DONE");
                tts.shutdown();
                return;
            }
            Job j = jobs.get(index);
            settings.setQuality(flutter, (double) j.flutter);
            settings.setSampleRate(j.sampleRate);
            tts.setSpeechRate(j.speed);
            started = SystemClock.elapsedRealtime();
            firstAudio = 0;
            tts.synthesizeToFile(TEXT, new Bundle(), file(j), String.valueOf(index));
        }

        @Override
        public void onStart(String id) {}

        @Override
        public void onAudioAvailable(String id, byte[] audio) {
            if (firstAudio == 0) firstAudio = SystemClock.elapsedRealtime();
        }

        @Override
        public void onDone(String id) {
            Job j = jobs.get(index);
            File f = file(j);
            long total = SystemClock.elapsedRealtime() - started;
            int headerRate = wavRate(f);
            if (j.speed == 1.0f && j.sampleRate == Settings.DEFAULT_SAMPLE_RATE) {
                normalSpeedFiles++;
                normalSpeedDigests.add(digest(f));
            }
            boolean rateOk = headerRate == j.sampleRate;
            Log.i(TAG, (rateOk ? "OK " : "FAIL ") + "flutter " + j.flutter + " x" + (int) j.speed + " at " + j.sampleRate
                    + " Hz: " + f.length() + " bytes, file says " + headerRate + " Hz, first audio after "
                    + (firstAudio == 0 ? -1 : firstAudio - started) + " ms, all in " + total + " ms");
            next();
        }

        @Override
        public void onError(String id) {
            Log.e(TAG, "FAIL utterance " + id);
            next();
        }

        private static int wavRate(File f) {
            byte[] h = new byte[28];
            try {
                FileInputStream in = new FileInputStream(f);
                try {
                    if (in.read(h) < 28) return -1;
                } finally {
                    in.close();
                }
            } catch (IOException e) {
                return -1;
            }
            return (h[24] & 0xff) | (h[25] & 0xff) << 8 | (h[26] & 0xff) << 16 | (h[27] & 0xff) << 24;
        }

        private static String digest(File f) {
            try {
                MessageDigest md = MessageDigest.getInstance("SHA-1");
                return android.util.Base64.encodeToString(md.digest(Dictionaries.read(f)), android.util.Base64.NO_WRAP);
            } catch (Exception e) {
                return f.getName();
            }
        }
    }
}
