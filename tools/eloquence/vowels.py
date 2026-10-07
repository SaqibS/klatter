"""Per-vowel median F1-F3: Eloquence vs our rendering vs our table targets.

Eloquence is measured over each vowel's aligned span (align.tsv); ours over the
exact segment span (segs.tsv), so a gap between "ours measured" and "our target"
shows what coarticulation and the synthesizer do to the table values, and the
gap to Eloquence shows which targets to move. Only stressed content-word vowels
plus the reduced AX/IX are used, since unstressed full vowels drift toward schwa.

Usage: python vowels.py WORKDIR [candidate.wav] [min_eloq_dur_s=0]
"""
from common import (VOWELS, eloq_span, formants_at, is_stressed_content, load_align,
                    load_reference, load_segs, our_span, parse_args, vm)
import numpy as np


def main():
    workdir, candidate, extra = parse_args(__doc__)
    min_dur = float(extra[0]) if extra else 0.0
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    eloq, ours, target = {}, {}, {}
    for row, seg in zip(load_align(workdir), load_segs(workdir)):
        vowel = row["ph"]
        if vowel not in VOWELS or not (is_stressed_content(row) or vowel in ("AX", "IX")):
            continue
        e0, e1 = eloq_span(row)
        if e1 - e0 < min_dur:
            continue
        fe = formants_at(ref_x, ref_sr, e0, e1)
        fo = formants_at(ours_x, ours_sr, *our_span(seg))
        if fe is not None and fo is not None:
            eloq.setdefault(vowel, []).append(fe)
            ours.setdefault(vowel, []).append(fo)
            target[vowel] = (float(row["f1"]), float(row["f2"]), float(row["f3"]))
    print("vowel   n   Eloquence F1/F2/F3   ours measured F1/F2/F3   our target F1/F2/F3")
    for vowel in VOWELS:
        if vowel in eloq:
            e, o, t = np.median(eloq[vowel], 0), np.median(ours[vowel], 0), target[vowel]
            print(f"{vowel:4s} {len(eloq[vowel]):4d}   {e[0]:4.0f} {e[1]:4.0f} {e[2]:4.0f}"
                  f"       {o[0]:4.0f} {o[1]:4.0f} {o[2]:4.0f}           {t[0]:4.0f} {t[1]:4.0f} {t[2]:4.0f}")


if __name__ == "__main__":
    main()
