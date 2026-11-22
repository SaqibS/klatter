package com.dgomeze42.klatter;

import java.nio.charset.Charset;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Klatter's voice-quality settings, as the engine describes them: flutter, head
 * size, roughness, breathiness, open quotient, spectral tilt, the source levels,
 * accent size, baseline fall, and F4 to F6 with their bandwidths.
 */
final class Parameters {
    static final class Parameter {
        final int index;
        final String code;
        final String name;
        final String unit;
        final String help;
        final double min;
        final double max;
        final double defaultValue;
        final double step;

        Parameter(int index, String code, String name, String unit, String help, double[] numbers) {
            this.index = index;
            this.code = code;
            this.name = name;
            this.unit = unit;
            this.help = help;
            this.min = numbers[0];
            this.max = numbers[1];
            this.defaultValue = numbers[2];
            this.step = numbers[3];
        }

        double clamp(double value) {
            return Math.max(min, Math.min(max, value));
        }
    }

    static final Charset UTF8 = Charset.forName("UTF-8");

    /** UTF-8 bytes, in every Android version. */
    static byte[] utf8(String text) {
        try {
            return text.getBytes("UTF-8");
        } catch (java.io.UnsupportedEncodingException e) {
            throw new AssertionError(e);
        }
    }

    static final List<Parameter> ALL;

    static {
        List<Parameter> all = new ArrayList<Parameter>();
        int n = KlatterNative.parameterCount();
        for (int i = 0; i < n; i++) {
            String text = KlatterNative.parameterText(i);
            double[] numbers = KlatterNative.parameterNumbers(i);
            if (text == null || numbers == null) continue;
            String[] f = text.split("\t", 4);
            all.add(new Parameter(i, f[0], f[1], f.length > 2 ? f[2] : "", f.length > 3 ? f[3] : "", numbers));
        }
        ALL = Collections.unmodifiableList(all);
    }

    private Parameters() {}

    static Parameter named(String code) {
        for (Parameter p : ALL) if (p.code.equals(code)) return p;
        return null;
    }

    /** Every setting's value in order: the user's where changed, else the default. */
    static double[] array(Map<String, Double> custom) {
        double[] out = new double[ALL.size()];
        for (Parameter p : ALL) {
            Double v = custom.get(p.code);
            out[p.index] = v == null || v.isNaN() ? p.defaultValue : p.clamp(v);
        }
        return out;
    }

    /** One decimal at most, and none when it would be ".0". */
    static String shown(double value) {
        double tenths = Math.rint(value * 10) / 10;
        if (tenths == Math.rint(tenths)) return Long.toString((long) tenths);
        return String.format(Locale.US, "%.1f", tenths);
    }
}
