package com.dgomeze42.klatter;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Build;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

/**
 * Klatter's settings, shared by the settings screen and the speech service (one
 * process, so both see every change at once). Reading is cheap enough for every
 * utterance: SharedPreferences keeps them in memory.
 */
final class Settings {
    static final int MIN_WPM = 30, MAX_WPM = 1920, NORMAL_WPM = 219;
    static final int MIN_PITCH = 40, MAX_PITCH = 400, NORMAL_PITCH = 100;
    static final int MAX_INFLECTION = 200, MAX_VOLUME = 200;
    /** Klatter's own rate. */
    static final int DEFAULT_SAMPLE_RATE = 16000;
    /** Every rate the engine is tuned for, the same twenty as on Windows and the Mac. */
    static final int[] SAMPLE_RATES = {
            8000, 10000, 11000, 11025, 12000, 16000, 20000, 22000, 22050, 24000,
            32000, 40000, 44000, 44100, 48000, 64000, 80000, 88000, 88200, 96000,
    };

    private final SharedPreferences prefs;
    private final Context context;

    Settings(Context context) {
        this.context = context.getApplicationContext();
        prefs = this.context.getSharedPreferences("klatter", Context.MODE_PRIVATE);
    }

    /**
     * Saves a change. On Android 2.2 to 3.2 Klatter is a plugin in Android's own
     * speech service, which cannot read these settings, so they are also written to
     * files/klatter.conf for it (rust/src/legacy.rs reads it).
     */
    @SuppressWarnings("deprecation")
    private void save(SharedPreferences.Editor editor) {
        if (Build.VERSION.SDK_INT >= 9) editor.apply();
        else editor.commit();
        if (Build.VERSION.SDK_INT < 14) exportForOldAndroid();
    }

    /** The settings file Android 2.2 to 3.2's engine plugin reads, readable by its process. */
    @SuppressWarnings("deprecation")
    void exportForOldAndroid() {
        StringBuilder b = new StringBuilder("# Written by the Klatter app for the engine plugin; changes here are replaced.\n");
        b.append("rate = ").append(rateWpm()).append('\n');
        b.append("follow-rate = ").append(followSystemRate() ? "yes" : "no").append('\n');
        b.append("pitch = ").append(exactPitch() ? String.valueOf(pitchHz()) : "natural").append('\n');
        b.append("follow-pitch = ").append(followSystemPitch() ? "yes" : "no").append('\n');
        b.append("inflection = ").append(inflection()).append('\n');
        b.append("volume = ").append(volume()).append('\n');
        b.append("sample-rate = ").append(sampleRate()).append('\n');
        for (Map.Entry<String, Double> e : quality().entrySet()) {
            b.append(e.getKey()).append(" = ").append(Parameters.shown(e.getValue())).append('\n');
        }
        for (DictionaryEntry d : dictionaries()) {
            if (d.enabled) b.append("dictionary = ").append(Dictionaries.file(context, d.file).getPath()).append('\n');
        }
        try {
            java.io.FileOutputStream out = context.openFileOutput("klatter.conf", Context.MODE_WORLD_READABLE);
            try {
                out.write(Parameters.utf8(b.toString()));
            } finally {
                out.close();
            }
        } catch (java.io.IOException ignored) {
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }

    private int getInt(String key, int def, int lo, int hi) {
        return clamp(prefs.getInt(key, def), lo, hi);
    }

    private void put(String key, int value) {
        save(prefs.edit().putInt(key, value));
    }

    private void put(String key, boolean value) {
        save(prefs.edit().putBoolean(key, value));
    }

    int rateWpm() { return getInt("rateWpm", NORMAL_WPM, MIN_WPM, MAX_WPM); }
    void setRateWpm(int v) { put("rateWpm", clamp(v, MIN_WPM, MAX_WPM)); }
    /** Let the screen reader's rate scale the rate. */
    boolean followSystemRate() { return prefs.getBoolean("followRate", true); }
    void setFollowSystemRate(boolean v) { put("followRate", v); }

    boolean exactPitch() { return prefs.getBoolean("exactPitch", false); }
    void setExactPitch(boolean v) { put("exactPitch", v); }
    int pitchHz() { return getInt("pitchHz", NORMAL_PITCH, MIN_PITCH, MAX_PITCH); }
    void setPitchHz(int v) { put("pitchHz", clamp(v, MIN_PITCH, MAX_PITCH)); }
    boolean followSystemPitch() { return prefs.getBoolean("followPitch", true); }
    void setFollowSystemPitch(boolean v) { put("followPitch", v); }

    int inflection() { return getInt("inflection", 100, 0, MAX_INFLECTION); }
    void setInflection(int v) { put("inflection", clamp(v, 0, MAX_INFLECTION)); }
    int volume() { return getInt("volume", 100, 0, MAX_VOLUME); }
    void setVolume(int v) { put("volume", clamp(v, 0, MAX_VOLUME)); }
    /**
     * The highest rate this Android's audio output can play: 96 kHz from Android
     * 5.0, whose audio tracks take up to that, and 48 kHz before, a limit of the
     * system itself. Every one of the engine's twenty rates up to it is offered.
     */
    static int highestSampleRate() {
        return Build.VERSION.SDK_INT >= 21 ? 96000 : 48000;
    }

    /** The rates offered on this Android: all twenty from 5.0 on, up to 48000 hertz before. */
    static int[] sampleRates() {
        int highest = highestSampleRate(), n = 0;
        for (int r : SAMPLE_RATES) if (r <= highest) n++;
        int[] out = new int[n];
        int i = 0;
        for (int r : SAMPLE_RATES) if (r <= highest) out[i++] = r;
        return out;
    }

    int sampleRate() {
        int r = prefs.getInt("sampleRate", DEFAULT_SAMPLE_RATE);
        if (r > highestSampleRate()) return highestSampleRate();
        for (int s : SAMPLE_RATES) if (s == r) return r;
        return DEFAULT_SAMPLE_RATE;
    }
    void setSampleRate(int v) { put("sampleRate", v); }

    // ---- Voice quality -------------------------------------------------------------

    /** The voice-quality settings the user changed, by code; the rest are the engine's defaults. */
    Map<String, Double> quality() {
        Map<String, Double> out = new HashMap<String, Double>();
        String json = prefs.getString("quality", null);
        if (json == null) return out;
        try {
            JSONObject o = new JSONObject(json);
            Iterator<String> keys = o.keys();
            while (keys.hasNext()) {
                String code = keys.next();
                Parameters.Parameter p = Parameters.named(code);
                double v = o.optDouble(code, Double.NaN);
                if (p != null && !Double.isNaN(v) && !Double.isInfinite(v)) out.put(code, p.clamp(v));
            }
        } catch (JSONException ignored) {
        }
        return out;
    }

    double value(Parameters.Parameter p) {
        Double v = quality().get(p.code);
        return v != null ? v : p.defaultValue;
    }

    /** Sets one, or with null (or its default) puts it back to the default. */
    void setQuality(Parameters.Parameter p, Double value) {
        Map<String, Double> all = quality();
        if (value == null || value == p.defaultValue) all.remove(p.code);
        else all.put(p.code, p.clamp(value));
        if (all.isEmpty()) {
            resetQuality();
            return;
        }
        JSONObject o = new JSONObject();
        try {
            for (Map.Entry<String, Double> e : all.entrySet()) o.put(e.getKey(), e.getValue().doubleValue());
        } catch (JSONException ignored) {
        }
        save(prefs.edit().putString("quality", o.toString()));
    }

    void resetQuality() {
        save(prefs.edit().remove("quality"));
    }

    // ---- Pronunciation dictionaries ----------------------------------------------

    static final class DictionaryEntry {
        final String file;
        final boolean enabled;

        DictionaryEntry(String file, boolean enabled) {
            this.file = file;
            this.enabled = enabled;
        }
    }

    /** The dictionaries in load order: a later one wins for the same word. */
    List<DictionaryEntry> dictionaries() {
        List<DictionaryEntry> out = new ArrayList<DictionaryEntry>();
        try {
            JSONArray a = new JSONArray(prefs.getString("dictionaries", "[]"));
            for (int i = 0; i < a.length(); i++) {
                JSONObject o = a.getJSONObject(i);
                out.add(new DictionaryEntry(o.getString("file"), o.optBoolean("enabled", true)));
            }
        } catch (JSONException ignored) {
        }
        return out;
    }

    void setDictionaries(List<DictionaryEntry> list) {
        JSONArray a = new JSONArray();
        try {
            for (DictionaryEntry d : list) a.put(new JSONObject().put("file", d.file).put("enabled", d.enabled));
        } catch (JSONException ignored) {
        }
        save(prefs.edit().putString("dictionaries", a.toString())
                .putInt("dictionaryVersion", dictionaryVersion() + 1));
    }

    /** Moves on whenever the dictionaries change, so the engine reloads them. */
    int dictionaryVersion() { return prefs.getInt("dictionaryVersion", 0); }
}
