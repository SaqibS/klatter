"""Per-consonant comparison of Eloquence and our rendering.

Sonorants (W Y R L M N NG) are compared by F1-F3; obstruents by band levels in
dB relative to each file's median stressed-vowel level, which cancels overall
gain differences so the numbers say how loud each consonant is against vowels.
Only the central fraction of each segment is measured, and very short Eloquence
segments are skipped because alignment error dominates them.

Usage: python cons.py WORKDIR [candidate.wav] [centre_frac=0.3] [min_eloq_dur_s=0.07]
"""
import numpy as np

from common import (FULL_VOWELS, eloq_span, formants_at, is_stressed_content, load_align,
                    load_reference, middle, our_span, parse_args, vm)

BANDS = [(100, 1000), (1000, 2000), (2000, 3000), (3000, 4000), (4000, 5000), (5000, 5500)]
SONORANTS = "W Y R L M N NG".split()
OBSTRUENTS = "F V TH DH S Z SH ZH HH P B T D K G CH JH".split()


def band_levels(x, sr, t0, t1):
    """Mean power spectrum of 20 ms frames over [t0, t1], summed into BANDS (dB)."""
    win = int(0.02 * sr)
    spectra = []
    for t in np.arange(t0, max(t1 - 0.02, t0 + 0.001), 0.005):
        start = int(t * sr)
        if start + win > len(x):
            break
        spectra.append(np.abs(np.fft.rfft(x[start:start + win] * np.hanning(win))) ** 2)
    if not spectra:
        return None
    power, freqs = np.mean(spectra, 0), np.fft.rfftfreq(win, 1 / sr)
    return np.array([10 * np.log10(power[(freqs >= lo) & (freqs < hi)].sum() + 1e-12) for lo, hi in BANDS])


def total_db(levels):
    return 10 * np.log10(np.sum(10 ** (levels / 10)))


def main():
    workdir, candidate, extra = parse_args(__doc__)
    frac = float(extra[0]) if len(extra) > 0 else 0.3
    min_dur = float(extra[1]) if len(extra) > 1 else 0.07
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    vowel_eloq, vowel_ours, eloq, ours = [], [], {}, {}
    for row in load_align(workdir):
        phone = row["ph"]
        e0, e1 = eloq_span(row)
        o0, o1 = our_span(row)
        if phone in FULL_VOWELS and is_stressed_content(row):
            a = band_levels(ref_x, ref_sr, *middle(e0, e1, 0.5))
            b = band_levels(ours_x, ours_sr, *middle(o0, o1, 0.5))
            if a is not None and b is not None:
                vowel_eloq.append(total_db(a))
                vowel_ours.append(total_db(b))
            continue
        if e1 - e0 < min_dur:
            continue
        if phone in SONORANTS:
            measure = formants_at
        elif phone in OBSTRUENTS:
            measure = band_levels
        else:
            continue
        a = measure(ref_x, ref_sr, *middle(e0, e1, frac))
        b = measure(ours_x, ours_sr, *middle(o0, o1, frac))
        if a is not None and b is not None:
            eloq.setdefault(phone, []).append(a)
            ours.setdefault(phone, []).append(b)
    ref_eloq, ref_ours = np.median(vowel_eloq), np.median(vowel_ours)

    print("sonorants: Eloquence F1/F2/F3 | ours")
    for phone in SONORANTS:
        if phone in eloq:
            e, o = np.median(eloq[phone], 0), np.median(ours[phone], 0)
            print(f"{phone:3s} {len(eloq[phone]):4d}  {e[0]:4.0f} {e[1]:4.0f} {e[2]:4.0f} | {o[0]:4.0f} {o[1]:4.0f} {o[2]:4.0f}")
    print("obstruents: band dB re vowel, Eloquence | ours | diff; bands", BANDS)
    for phone in OBSTRUENTS:
        if phone in eloq:
            e = np.median(eloq[phone], 0) - ref_eloq
            o = np.median(ours[phone], 0) - ref_ours
            print(f"{phone:3s} {len(eloq[phone]):4d}  " + " ".join(f"{v:4.0f}" for v in e) + " | "
                  + " ".join(f"{v:4.0f}" for v in o) + " | " + " ".join(f"{v:+4.0f}" for v in e - o))


if __name__ == "__main__":
    main()
