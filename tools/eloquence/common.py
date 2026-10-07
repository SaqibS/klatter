"""Shared helpers for the Eloquence comparison scripts.

Every script works on a WORKDIR holding:
  reference.wav        Eloquence reading of the text
  words.tsv            SAPI word timings of reference.wav (asr_words.ps1)
  segs.tsv             our segment dump for the same text (examples/segdump.rs)
  align.tsv            segs.tsv plus eloq_start/eloq_end columns (align.py)
  anchored_words.tsv   word anchors used by align.py (written by align.py)
and an optional candidate WAV (our rendering), defaulting to WORKDIR/candidate.wav.
"""
import csv
import os
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import voice_metrics as vm  # noqa: E402

FULL_VOWELS = "IY IH EY EH AE AA AO OW UH UW AH ER AY AW OY".split()
VOWELS = FULL_VOWELS + ["AX", "IX"]
LPC_RATE = 10000


def parse_args(usage):
    """Return (workdir, candidate_wav, extra_args) from the command line."""
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        sys.exit(usage)
    workdir = sys.argv[1]
    candidate = sys.argv[2] if len(sys.argv) > 2 else os.path.join(workdir, "candidate.wav")
    return workdir, candidate, sys.argv[3:]


def read_tsv(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def load_reference(workdir):
    return vm.load(os.path.join(workdir, "reference.wav"))


def load_align(workdir):
    return read_tsv(os.path.join(workdir, "align.tsv"))


def load_segs(workdir):
    return read_tsv(os.path.join(workdir, "segs.tsv"))


def our_span(row):
    """Segment start/end in seconds on our own timeline."""
    start = float(row["start_ms"]) / 1000
    return start, start + float(row["dur_ms"]) / 1000


def eloq_span(row):
    """Segment start/end in seconds on the Eloquence timeline (from align.tsv)."""
    return float(row["eloq_start"]), float(row["eloq_end"])


def is_stressed_content(row):
    """Stressed syllable of a content word: the most carefully articulated tokens."""
    return row["stress"] != "0" and row["fn"] == "0"


def middle(t0, t1, frac):
    """Central `frac` of [t0, t1], away from coarticulated edges."""
    centre, half = (t0 + t1) / 2, (t1 - t0) * frac / 2
    return centre - half, centre + half


def formants_at(x, sr, t0, t1):
    """Median F1-F3 over the middle half of [t0, t1] (25 ms frames every 5 ms),
    by 12-pole LPC at 10 kHz so both voices are measured the same way."""
    win = int(0.025 * sr)
    keep = LPC_RATE * win // sr
    a, b = t0 + (t1 - t0) * 0.25, t0 + (t1 - t0) * 0.75
    found = []
    for t in np.arange(a, max(b, a + 0.001), 0.005):
        start = int(t * sr) - win // 2
        if start < 0 or start + win > len(x):
            continue
        frame = x[start:start + win]
        if np.sqrt(np.mean(frame ** 2)) < 0.01:
            continue
        # Truncating the spectrum resamples to LPC_RATE without a filter design.
        spec = np.fft.rfft(frame * np.hanning(win))[:keep // 2 + 1]
        y = np.fft.irfft(spec, keep)
        y = np.append(y[0], y[1:] - 0.97 * y[:-1])
        roots = [r for r in np.roots(vm.lpc(y, 12)) if r.imag > 0]
        freqs = sorted(np.angle(r) * LPC_RATE / (2 * np.pi) for r in roots
                       if -LPC_RATE / np.pi * np.log(abs(r)) < 500
                       and np.angle(r) * LPC_RATE / (2 * np.pi) > 180)
        if len(freqs) >= 3:
            found.append(freqs[:3])
    return np.median(found, axis=0) if found else None
