"""Compare a rendered WAV against a reference voice using rate-independent metrics.

Usage: python tools/voice_metrics.py reference.wav candidate.wav [more.wav ...]
Requires numpy.
"""
import sys
import wave

import numpy as np

HOP_S = 0.01
WIN_S = 0.04
LPC_RATE = 10000
LTAS_BANDS = [(80, 250), (250, 500), (500, 1000), (1000, 2000), (2000, 3000),
              (3000, 4000), (4000, 5500), (5500, 8000)]

# Tolerances define what counts as one "unit" of mismatch in the score.
TOLERANCES = {
    "f0_median": 5.0, "f0_p5": 8.0, "f0_p95": 10.0, "f0_sd_st": 0.5,
    "h1_h2": 2.0, "f1_median": 30.0, "f2_median": 80.0, "f3_median": 120.0,
    "pauses_per_min": 5.0, "pause_mean_ms": 50.0, "pause_fraction": 0.03,
}
for lo, hi in LTAS_BANDS:
    TOLERANCES[f"ltas_{lo}_{hi}"] = 3.0

UNITS = {"f0_median": "Hz", "f0_p5": "Hz", "f0_p95": "Hz", "f0_sd_st": "st",
         "h1_h2": "dB", "f1_median": "Hz", "f2_median": "Hz", "f3_median": "Hz",
         "pauses_per_min": "/min", "pause_mean_ms": "ms", "pause_fraction": ""}


def load(path):
    with wave.open(path) as w:
        sr, ch, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = w.readframes(w.getnframes())
    if width != 2:
        raise ValueError(f"{path}: only 16-bit PCM is supported")
    x = np.frombuffer(raw, dtype=np.int16).astype(np.float64)
    if ch > 1:
        x = x.reshape(-1, ch).mean(axis=1)
    return x / (np.abs(x).max() + 1e-9), sr


def frames(x, sr):
    win, hop = int(WIN_S * sr), int(HOP_S * sr)
    return np.lib.stride_tricks.sliding_window_view(x, win)[::hop]


def speech_mask(fr):
    db = 10 * np.log10(np.mean(fr ** 2, axis=1) + 1e-12)
    return db > np.percentile(db, 95) - 30


def pause_stats(speech):
    # Gaps under 100 ms are stop closures, not prosodic breaks.
    edges = np.diff(np.concatenate([[1], speech.astype(int), [1]]))
    starts, ends = np.where(edges == -1)[0], np.where(edges == 1)[0]
    lengths = (ends - starts)[1:-1] if len(starts) > 2 else np.array([])
    lengths = lengths[lengths * HOP_S >= 0.1]
    speech_min = speech.sum() * HOP_S / 60
    return {
        "pauses_per_min": len(lengths) / speech_min,
        "pause_mean_ms": lengths.mean() * HOP_S * 1000 if len(lengths) else 0.0,
        "pause_fraction": lengths.sum() / len(speech),
    }


def pitch(fr, sr):
    win = fr.shape[1]
    w = fr * np.hanning(win)
    w = w - w.mean(axis=1, keepdims=True)
    spec = np.fft.rfft(w, 2 * win)
    ac = np.fft.irfft(np.abs(spec) ** 2)[:, :win]
    ac = ac / (ac[:, :1] + 1e-12)
    lo, hi = int(sr / 300), int(sr / 60)
    lag = lo + np.argmax(ac[:, lo:hi], axis=1)
    strength = ac[np.arange(len(lag)), lag]
    f0 = sr / lag
    # Autocorrelation makes isolated octave errors; true pitch moves smoothly.
    padded = np.pad(f0, 5, mode="edge")
    local = np.median(np.lib.stride_tricks.sliding_window_view(padded, 11), axis=1)
    smooth = np.abs(np.log2(f0 / local)) < 0.4
    return f0, (strength > 0.5) & smooth


def h1_h2(fr, sr, f0):
    n = fr.shape[1] * 2
    mag = 20 * np.log10(np.abs(np.fft.rfft(fr * np.hanning(fr.shape[1]), n)) + 1e-12)
    bin_hz = sr / n
    out = []
    for m, f in zip(mag, f0):
        def peak(target):
            a, b = int(target * 0.9 / bin_hz), int(target * 1.1 / bin_hz) + 1
            return m[a:b].max()
        out.append(peak(f) - peak(2 * f))
    return np.array(out)


def lpc(y, order):
    r = np.correlate(y, y, "full")[len(y) - 1:len(y) + order]
    a, err = np.array([1.0]), r[0]
    for i in range(1, order + 1):
        k = -(r[i] + a[1:] @ r[i - 1:0:-1]) / err
        a = np.concatenate([a, [0.0]]) + k * np.concatenate([[0.0], a[::-1]])
        err *= 1 - k * k
    return a


def formants(fr, sr):
    keep = LPC_RATE * fr.shape[1] // sr
    out = [[], [], []]
    for f in fr:
        spec = np.fft.rfft(f * np.hanning(len(f)))[:keep // 2 + 1]
        y = np.fft.irfft(spec, keep)
        y = np.append(y[0], y[1:] - 0.97 * y[:-1])
        roots = [r for r in np.roots(lpc(y, 12)) if r.imag > 0]
        freqs = sorted(np.angle(r) * LPC_RATE / (2 * np.pi) for r in roots
                       if -LPC_RATE / np.pi * np.log(abs(r)) < 400
                       and np.angle(r) * LPC_RATE / (2 * np.pi) > 200)
        for i in range(min(3, len(freqs))):
            out[i].append(freqs[i])
    return [np.median(v) if v else np.nan for v in out]


def ltas(fr, sr):
    power = (np.abs(np.fft.rfft(fr * np.hanning(fr.shape[1]), axis=1)) ** 2).mean(axis=0)
    db = 10 * np.log10(power + 1e-20)
    freq = np.fft.rfftfreq(fr.shape[1], 1 / sr)
    ref = db[(freq >= 250) & (freq < 500)].mean()
    return {f"ltas_{lo}_{hi}": db[(freq >= lo) & (freq < hi)].mean() - ref
            for lo, hi in LTAS_BANDS}


def analyze(path):
    x, sr = load(path)
    if sr < 16000:
        raise ValueError(f"{path}: sample rate must be at least 16 kHz")
    fr = frames(x, sr)
    speech = speech_mask(fr)
    f0, voiced = pitch(fr, sr)
    voiced &= speech
    f0v = f0[voiced]
    vfr = fr[voiced]
    # LPC per frame is slow in numpy; a few thousand frames give stable medians.
    stride = max(1, len(vfr) // 4000)
    f1, f2, f3 = formants(vfr[::stride], sr)
    st = 12 * np.log2(f0v / np.median(f0v))
    m = {
        "f0_median": np.median(f0v), "f0_p5": np.percentile(f0v, 5),
        "f0_p95": np.percentile(f0v, 95), "f0_sd_st": st.std(),
        "h1_h2": np.median(h1_h2(vfr[::stride], sr, f0v[::stride])),
        "f1_median": f1, "f2_median": f2, "f3_median": f3,
    }
    m.update(pause_stats(speech))
    m.update(ltas(fr[speech], sr))
    return m


def score(ref, cand):
    z = [(cand[k] - ref[k]) / t for k, t in TOLERANCES.items()]
    return float(np.sqrt(np.mean(np.square(z))))


def label(key):
    if key.startswith("ltas_"):
        lo, hi = key.split("_")[1:]
        return f"Spectrum {lo}-{hi} Hz (dB re 250-500)", "dB"
    return key.replace("_", " "), UNITS.get(key, "")


def main(paths):
    if len(paths) < 2:
        sys.exit(__doc__)
    ref = analyze(paths[0])
    for path in paths[1:]:
        cand = analyze(path)
        print(f"{path}: score {score(ref, cand):.2f} (0 = matches reference)")
        worst = sorted(TOLERANCES, key=lambda k: -abs(cand[k] - ref[k]) / TOLERANCES[k])
        for k in worst:
            name, unit = label(k)
            diff = cand[k] - ref[k]
            print(f"- {name}: {cand[k]:.2f} vs {ref[k]:.2f} {unit}, "
                  f"diff {diff:+.2f}, {abs(diff) / TOLERANCES[k]:.1f} tolerances")
        print()


if __name__ == "__main__":
    main(sys.argv[1:])
