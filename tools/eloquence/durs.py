"""Vowel and fricative durations from acoustic landmarks, measured identically
on Eloquence and on ours.

Segment boundaries from alignment are too coarse to compare durations directly,
so each segment's extent is re-measured from the signal: a vowel is the run of
frames where low-band (100-1000 Hz) energy dominates and stays within 10 dB of
its peak; a fricative the run where high-band (2500-5000 Hz) energy does. Only
clean contexts are used (vowel between obstruents, S SH Z F between vowels), so
edges are sharp. The search window is the segment span +-50 ms: the aligned
span on Eloquence, the true span on ours. Ratios >1 mean Eloquence is longer.

Usage: python durs.py WORKDIR [candidate.wav]
"""
import numpy as np

from common import VOWELS, eloq_span, load_align, load_reference, our_span, parse_args, vm

HOP = 0.0025
OBSTRUENTS = set("P T K B D G CH JH F V TH DH S Z SH ZH HH".split())
FRICATIVES = {"S", "SH", "Z", "F"}


def band_energies(x, sr):
    """Per-frame low (100-1000 Hz) and high (2500-5000 Hz) band levels in dB."""
    win, hop = int(0.015 * sr), int(HOP * sr)
    frames = np.lib.stride_tricks.sliding_window_view(x, win)[::hop] * np.hanning(win)
    power, freqs = np.abs(np.fft.rfft(frames, axis=1)) ** 2, np.fft.rfftfreq(win, 1 / sr)

    def band_db(lo, hi):
        return 10 * np.log10(power[:, (freqs >= lo) & (freqs < hi)].sum(1) + 1e-12)
    return band_db(100, 1000), band_db(2500, 5000)


def landmark_ms(low, high, t0, t1, kind):
    """Length of the contiguous vowel-like ('v') or fricative-like ('f') run
    around the strongest frame in the central 30-70 % of [t0, t1]."""
    a, b = int((t0 - 0.05) / HOP), int((t1 + 0.05) / HOP)
    c0 = int(t0 / HOP) + int((t1 - t0) / HOP * 0.3)
    c1 = int(t0 / HOP) + int((t1 - t0) / HOP * 0.7) + 1
    if a < 0 or b >= len(low) or c1 <= c0:
        return None
    level = low if kind == "v" else high
    dominant = (low - high > 0) if kind == "v" else (high - low > -5)
    inside = (level > level[c0:c1].max() - 10) & dominant
    peak = c0 + int(np.argmax(level[c0:c1]))
    if not inside[peak]:
        return None
    i = j = peak
    while i > a and inside[i - 1]:
        i -= 1
    while j < b and inside[j + 1]:
        j += 1
    # A run reaching the window edge has no measurable boundary.
    if i == a or j == b:
        return None
    return (j - i + 1) * HOP * 1000


def context_class(row):
    if row["fn"] == "1":
        return "fn"
    if row["final"] == "1":
        return "final"
    return "str" if row["stress"] != "0" else "unstr"


def main():
    workdir, candidate, _ = parse_args(__doc__)
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    eloq_low, eloq_high = band_energies(ref_x, ref_sr)
    ours_low, ours_high = band_energies(ours_x, ours_sr)
    rows = load_align(workdir)
    durations = {}
    for k in range(1, len(rows) - 1):
        row, prev, nxt = rows[k], rows[k - 1]["ph"], rows[k + 1]["ph"]
        phone = row["ph"]
        if phone in VOWELS and prev in OBSTRUENTS and nxt in OBSTRUENTS:
            kind = "v"
        elif phone in FRICATIVES and prev in VOWELS and nxt in VOWELS:
            kind = "f"
        else:
            continue
        eloq_ms = landmark_ms(eloq_low, eloq_high, *eloq_span(row), kind)
        ours_ms = landmark_ms(ours_low, ours_high, *our_span(row), kind)
        if eloq_ms and ours_ms:
            ctx = context_class(row)
            durations.setdefault((phone, ctx), []).append((eloq_ms, ours_ms))
            durations.setdefault(("ALL" if kind == "v" else "FRIC", ctx), []).append((eloq_ms, ours_ms))
    print("phone ctx n  Eloquence_ms ours_ms ratio")
    for key in sorted(durations):
        pairs = np.array(durations[key])
        if len(pairs) >= 5:
            print(f"{key[0]:4s} {key[1]:5s} {len(pairs):3d}  {np.median(pairs[:, 0]):5.0f} {np.median(pairs[:, 1]):5.0f}"
                  f"  {np.median(pairs[:, 0] / pairs[:, 1]):.2f}")


if __name__ == "__main__":
    main()
