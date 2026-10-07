"""Vowel spectral envelope residual: band levels of stressed vowels, Eloquence
(E) vs ours (O), in dB relative to the 250-500 Hz band.

Formant frequencies alone miss spectral tilt, bandwidths and the balance between
formant regions; normalizing each vowel to its F1-region band shows those
differences independent of overall loudness. Measured over the middle half of
each stressed content-word vowel, 30 ms frames every 5 ms.

Usage: python vresid.py WORKDIR [candidate.wav]
"""
import numpy as np

from common import (eloq_span, is_stressed_content, load_align, load_reference, middle,
                    our_span, parse_args, vm)

EDGES = [100, 250, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000]
MEASURED_VOWELS = ("IY", "EH", "AE", "AA", "AO", "OW", "UW", "AH", "IH")


def band_power(x, sr, t0, t1):
    """Mean power per EDGES band over [t0, t1], or None if the span is too short."""
    win = int(0.03 * sr)
    spectra = []
    for t in np.arange(t0, t1 - 0.03, 0.005):
        start = int(t * sr)
        spectra.append(np.abs(np.fft.rfft(x[start:start + win] * np.hanning(win))) ** 2)
    if not spectra:
        return None
    power, freqs = np.mean(spectra, 0), np.fft.rfftfreq(win, 1 / sr)
    return np.array([power[(freqs >= lo) & (freqs < hi)].mean() for lo, hi in zip(EDGES, EDGES[1:])])


def main():
    workdir, candidate, _ = parse_args(__doc__)
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    by_vowel = {}
    for row in load_align(workdir):
        if row["ph"] not in MEASURED_VOWELS or not is_stressed_content(row):
            continue
        eloq = band_power(ref_x, ref_sr, *middle(*eloq_span(row), 0.5))
        ours = band_power(ours_x, ours_sr, *middle(*our_span(row), 0.5))
        if eloq is not None and ours is not None and eloq[1] > 0 and ours[1] > 0:
            by_vowel.setdefault(row["ph"], []).append(
                np.concatenate([10 * np.log10(eloq / eloq[1] + 1e-12), 10 * np.log10(ours / ours[1] + 1e-12)]))
    n_bands = len(EDGES) - 1
    print("band Hz:", " ".join(f"{lo}-{hi}" for lo, hi in zip(EDGES, EDGES[1:])))
    for vowel, levels in by_vowel.items():
        median = np.median(levels, 0)
        print(f"{vowel:3s} E " + " ".join(f"{v:+5.0f}" for v in median[:n_bands]))
        print("    O " + " ".join(f"{v:+5.0f}" for v in median[n_bands:]))


if __name__ == "__main__":
    main()
