"""Map each of our segments onto the Eloquence timeline.

Eloquence and our voice differ too much for frame-level DTW to stay on track
over a long reading, so alignment is anchored on words: SAPI word timings of
the reference (words.tsv) are matched to our word sequence (segs.tsv), and only
words inside runs of 3+ consecutive matches are trusted, because isolated
matches are often coincidences ("the" pairing with the wrong "the"). Between
anchors, time is mapped linearly, which tracks sibilants better than in-word DTW.

Writes WORKDIR/align.tsv (segs.tsv plus eloq_start/eloq_end, seconds) and
WORKDIR/anchored_words.tsv (our_start, our_end, eloq_start, eloq_end per
anchored word). As a sanity check, prints the share of /s sh z/ segments whose
aligned span reaches a high-centroid (>2.5 kHz) Eloquence frame.

Usage: python align.py WORKDIR [candidate.wav]
"""
import csv
import difflib
import os

import numpy as np

from common import load_reference, load_segs, our_span, parse_args, read_tsv, vm


def reference_spans(rows, anchors):
    """Map exact segment boundaries through the monotonic word anchors."""
    ours, reference = np.array(anchors).T
    return np.interp(np.array([our_span(row) for row in rows]), ours, reference)


def our_word_spans(rows):
    """(word, start_s, end_s) for every word in the segment dump."""
    words = []
    for i, row in enumerate(rows):
        if row["wi"] == "1" and row["ph"] != "_":
            j = i + 1
            while j < len(rows) and rows[j]["wi"] != "1" and rows[j]["ph"] != "_":
                j += 1
            last = rows[j - 1]
            end = float(last["start_ms"]) / 1000 + float(last["dur_ms"]) / 1000
            words.append((row["word"].lower(), float(row["start_ms"]) / 1000, end))
    return words


def main():
    workdir, candidate, _ = parse_args(__doc__)
    rows = load_segs(workdir)
    our_words = our_word_spans(rows)
    with open(os.path.join(workdir, "words.tsv")) as f:
        asr_rows = [line.rstrip("\n").split("\t") for line in f]

    matcher = difflib.SequenceMatcher(None, [r[3].lower() for r in asr_rows],
                                      [w[0] for w in our_words], autojunk=False)
    pairs = [(asr_rows[blk.a + d], our_words[blk.b + d])
             for blk in matcher.get_matching_blocks() if blk.size >= 3
             for d in range(blk.size)]
    with open(os.path.join(workdir, "anchored_words.tsv"), "w") as f:
        f.write("".join(f"{word[1]:.3f}\t{word[2]:.3f}\t{asr[1]}\t{asr[2]}\n" for asr, word in pairs))
    print(f"recognized {len(asr_rows)}, anchored words {len(pairs)} of {len(our_words)}")

    ours_x, ours_sr = vm.load(candidate)
    ref_x, ref_sr = load_reference(workdir)
    # Anchors are (our time, Eloquence time); both files start and end together.
    anchors = [(0.0, 0.0)]
    for asr, word in pairs:
        anchors += [(word[1], float(asr[1])), (word[2], float(asr[2]))]
    anchors.append((len(ours_x) / ours_sr, len(ref_x) / ref_sr))
    monotonic = [anchors[0]]
    for point in anchors[1:]:
        if point[0] > monotonic[-1][0] and point[1] > monotonic[-1][1]:
            monotonic.append(point)
    anchors = monotonic
    gaps = np.diff([a[1] for a in anchors])
    print(f"anchors {len(anchors)}, eloq gap median {np.median(gaps):.2f}s max {gaps.max():.1f}s")

    align_path = os.path.join(workdir, "align.tsv")
    with open(align_path, "w", newline="") as out:
        writer = csv.writer(out, delimiter="\t")
        writer.writerow(list(rows[0].keys()) + ["eloq_start", "eloq_end"])
        for row, (start, end) in zip(rows, reference_spans(rows, anchors)):
            writer.writerow(list(row.values()) + [f"{start:.3f}", f"{end:.3f}"])

    win = int(0.02 * ref_sr)

    def centroid(t):
        a = min(max(0, int(t * ref_sr)), len(ref_x) - win)
        power = np.abs(np.fft.rfft(ref_x[a:a + win] * np.hamming(win))) ** 2
        return (power * np.fft.rfftfreq(win, 1 / ref_sr)).sum() / (power.sum() + 1e-12)

    hits = np.array([max(centroid(t) for t in np.arange(float(r["eloq_start"]), float(r["eloq_end"]) + 0.001, 0.005)) > 2500
                     for r in read_tsv(align_path) if r["ph"] in ("S", "SH", "Z")])
    print("sibilant span hits:", round(float(hits.mean()), 2),
          "by tenth", [round(float(h.mean()), 2) for h in np.array_split(hits, 10)])


if __name__ == "__main__":
    main()
