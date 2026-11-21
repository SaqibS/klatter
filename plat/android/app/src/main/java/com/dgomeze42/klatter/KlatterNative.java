package com.dgomeze42.klatter;

/**
 * The Klatter engine, in libklatter.so (../../jni and rust): the C interface of
 * ffi/klatter.h. Text crosses as UTF-8 bytes. Speaking is stateless, so any
 * thread may speak, one utterance per call.
 */
final class KlatterNative {
    static {
        // libklatter.so, built for Android 4.0 and newer. On Android 2.2 to 3.2,
        // and on ARMv5/ARMv6 phones, it is libttsklatter.so instead: the engine
        // plugin those versions load (jni/legacy_plugin.c), which carries the
        // same JNI functions, built for ARMv5 and Android 2.2.
        try {
            System.loadLibrary("klatter");
        } catch (UnsatisfiedLinkError e) {
            System.loadLibrary("ttsklatter");
        }
    }

    private KlatterNative() {}

    static final int OK = 0;
    static final int STOPPED = 1;

    /** Receives 16-bit little-endian mono PCM; returns 0 to go on, anything else to stop. */
    interface AudioSink {
        int onAudio(byte[] pcm, int length);
    }

    static native int parameterCount();
    /** "code\tname\tunit\thelp", or null. */
    static native String parameterText(int index);
    /** {min, max, default, step}, or null. */
    static native double[] parameterNumbers(int index);

    static native long dictionaryNew();
    /** 1 when the word and its ARPAbet sounds were taken, -1 when not. */
    static native int dictionarySet(long dictionary, byte[] word, byte[] phones);
    static native void dictionaryFree(long dictionary);

    /**
     * Speaks text; returns OK, STOPPED, or a negative error.
     *
     * @param quality the voice-quality settings in Parameters.ALL's order
     * @param dictionary a dictionary from dictionaryNew, or 0
     */
    static native int speak(byte[] utf8, double wpm, double pitchHz, double inflection, double volume,
                            int sampleRate, double[] quality, long dictionary, int chunkBytes, AudioSink sink);

    /**
     * Loads a dictionary's text: lines of a word and its ARPAbet sounds ("aback AH0
     * B AE1 K"); # starts a comment. Returns {taken in, left out, first left-out line}.
     */
    static int[] dictionaryLoad(long dictionary, byte[] utf8) {
        String text = new String(utf8, Parameters.UTF8);
        int added = 0, rejected = 0, first = 0, number = 0;
        for (String line : text.split("\r?\n|\r")) {
            number++;
            int hash = line.indexOf('#');
            if (hash >= 0) line = line.substring(0, hash);
            line = line.trim();
            if (line.length() == 0) continue;
            String[] parts = line.split("\\s+", 2);
            boolean ok = parts.length == 2
                    && dictionarySet(dictionary, Parameters.utf8(parts[0]), Parameters.utf8(parts[1].trim())) == 1;
            if (ok) {
                added++;
            } else {
                rejected++;
                if (first == 0) first = number;
            }
        }
        return new int[]{added, rejected, first};
    }
}
