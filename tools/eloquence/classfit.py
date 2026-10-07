"""Duration scale per phone class, fitted from anchored word durations.

Each anchored word gives one equation: Eloquence word duration = sum over
classes of (our time spent in that class within the word) x scale. Least
squares over all anchored words yields one scale per class, i.e. how much
longer (>1) or shorter Eloquence makes stressed vowels, stops, nasals, etc.
Uses segs.tsv and anchored_words.tsv (written by align.py); no audio needed.

Usage: python classfit.py WORKDIR
"""
import os

import numpy as np

from common import VOWELS, load_segs, parse_args

CLASSES = ["strV", "unsV", "fnV", "stop", "fric", "nasal", "liqgl"]
STOPS = "P T K B D G CH JH".split()
FRICATIVES = "F V TH DH S Z SH ZH HH".split()
NASALS = "M N NG".split()


def phone_class(row):
    phone = row["ph"]
    if phone in VOWELS:
        return "fnV" if row["fn"] == "1" else ("strV" if row["stress"] != "0" else "unsV")
    if phone in STOPS:
        return "stop"
    if phone in FRICATIVES:
        return "fric"
    if phone in NASALS:
        return "nasal"
    return "liqgl"


def main():
    workdir, _, _ = parse_args(__doc__)
    segs = load_segs(workdir)
    class_time, eloq_dur = [], []
    with open(os.path.join(workdir, "anchored_words.tsv")) as f:
        for line in f:
            o0, o1, e0, e1 = map(float, line.split())
            per_class = np.zeros(len(CLASSES))
            for row in segs:
                start = float(row["start_ms"]) / 1000
                if row["ph"] != "_" and o0 - 1e-6 <= start < o1 - 1e-6:
                    per_class[CLASSES.index(phone_class(row))] += float(row["dur_ms"]) / 1000
            class_time.append(per_class)
            eloq_dur.append(e1 - e0)
    class_time, eloq_dur = np.array(class_time), np.array(eloq_dur)
    scales, *_ = np.linalg.lstsq(class_time, eloq_dur, rcond=None)
    print("words", len(eloq_dur), "eloq/ours total", round(eloq_dur.sum() / class_time.sum(), 2))
    for name, scale, total in zip(CLASSES, scales, class_time.sum(0)):
        print(f"{name:6s} scale {scale:.2f}  (our total {total:.0f}s)")


if __name__ == "__main__":
    main()
