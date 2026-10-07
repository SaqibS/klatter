"""Intrinsic vowel level: each vowel's median level relative to the
token-weighted mean vowel level, Eloquence vs ours.

Open vowels are naturally louder than close ones; if our synthesizer's
per-vowel amplitudes differ from Eloquence's pattern, some vowels stand out or
sink. Level is mean power over the central 30-70 % of each vowel; vowels whose
segment is shorter than 60 ms are skipped as mostly transition.

Output columns: vowel, n, Eloquence dB, ours dB, difference (E - O).

Usage: python vlevel.py WORKDIR [candidate.wav]
"""
import collections

import numpy as np

from common import VOWELS, eloq_span, load_align, load_reference, our_span, parse_args, vm


def level_db(x, sr, t0, t1):
    y = x[int((t0 + 0.3 * (t1 - t0)) * sr):int((t0 + 0.7 * (t1 - t0)) * sr)]
    power = np.mean(y ** 2)
    return 10 * np.log10(power) if power > 0 else None


def main():
    workdir, candidate, _ = parse_args(__doc__)
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    eloq, ours = collections.defaultdict(list), collections.defaultdict(list)
    for row in load_align(workdir):
        vowel = row["ph"]
        if vowel not in VOWELS or float(row["dur_ms"]) / 1000 < 0.06 or not row["eloq_start"]:
            continue
        e = level_db(ref_x, ref_sr, *eloq_span(row))
        o = level_db(ours_x, ours_sr, *our_span(row))
        if e is not None and o is not None:
            eloq[vowel].append(e)
            ours[vowel].append(o)
    counts = {v: len(eloq[v]) for v in eloq}
    eloq_med = {v: np.median(eloq[v]) for v in eloq}
    ours_med = {v: np.median(ours[v]) for v in ours}
    total = sum(counts.values())
    eloq_mean = sum(eloq_med[v] * counts[v] for v in counts) / total
    ours_mean = sum(ours_med[v] * counts[v] for v in counts) / total
    print("vowel\tn\tEloq\tours\tdiff")
    for vowel in VOWELS:
        if vowel in counts and counts[vowel] >= 5:
            e, o = eloq_med[vowel] - eloq_mean, ours_med[vowel] - ours_mean
            print(f"{vowel}\t{counts[vowel]}\t{e:+.1f}\t{o:+.1f}\t{e - o:+.1f}")


if __name__ == "__main__":
    main()
