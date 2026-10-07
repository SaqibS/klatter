"""Boundary abruptness: spectral flux at segment transitions, by transition type.

At each boundary between two segments, takes the largest frame-to-frame change
in band levels (dB, 20 ms frames every 5 ms) separately for low bands
(100-2200 Hz) and high bands (2200-5400 Hz). Formant synthesizers tend to jump
where natural-sounding speech glides (or the reverse), and this shows where.
The Eloquence window is +-30 ms around the aligned boundary to absorb alignment
error; ours is +-15 ms around the exact boundary.

Usage: python bjump.py WORKDIR [candidate.wav]
"""
import collections

import numpy as np

from common import load_align, load_reference, load_segs, parse_args, vm

HOP = 0.005
BAND_EDGES = [100, 300, 600, 1000, 1500, 2200, 3000, 4000, 5400]
N_LOW_BANDS = 5


def band_flux(x, sr):
    """Per-frame mean absolute band-level change for low and high bands."""
    hop, win = int(HOP * sr), int(0.02 * sr)
    n_frames = (len(x) - win) // hop
    idx = np.arange(win)[None, :] + hop * np.arange(n_frames)[:, None]
    power = np.abs(np.fft.rfft(x[idx] * np.hanning(win), axis=1)) ** 2
    freqs = np.fft.rfftfreq(win, 1 / sr)
    levels = 10 * np.log10(np.stack([power[:, (freqs >= lo) & (freqs < hi)].sum(1)
                                     for lo, hi in zip(BAND_EDGES[:-1], BAND_EDGES[1:])], 1) + 1e-9)
    levels -= np.percentile(levels.max(1), 95)
    # Changes below the noise floor are inaudible and would dominate silences.
    levels = np.maximum(levels, -50)
    delta = np.abs(np.diff(levels, axis=0))
    return delta[:, :N_LOW_BANDS].mean(1), delta[:, N_LOW_BANDS:].mean(1)


def phone_class(phone):
    if phone in ("P", "T", "K"):
        return "vlstop"
    if phone in ("B", "D", "G"):
        return "vdstop"
    if phone in ("S", "Z", "SH", "ZH", "F", "V", "TH", "DH", "CH", "JH"):
        return "fric"
    if phone == "HH":
        return "h"
    if phone in ("M", "N", "NG", "L", "R", "W", "Y"):
        return "son"
    if phone == "_":
        return "pause"
    return "V"


def main():
    workdir, candidate, _ = parse_args(__doc__)
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    eloq_low, eloq_high = band_flux(ref_x, ref_sr)
    ours_low, ours_high = band_flux(ours_x, ours_sr)
    aligned, segs = load_align(workdir), load_segs(workdir)
    # Per transition: [eloq low, ours low, eloq high, ours high] peak flux values.
    by_transition = collections.defaultdict(lambda: [[], [], [], []])
    for i in range(1, min(len(aligned), len(segs))):
        if not aligned[i]["eloq_start"]:
            continue
        transition = phone_class(aligned[i - 1]["ph"]) + ">" + phone_class(aligned[i]["ph"])
        if "pause" in transition:
            continue
        t_eloq = float(aligned[i]["eloq_start"])
        t_ours = float(segs[i]["start_ms"]) / 1000
        e0, e1 = int((t_eloq - 0.03) / HOP), int((t_eloq + 0.03) / HOP)
        o0, o1 = int((t_ours - 0.015) / HOP), int((t_ours + 0.015) / HOP)
        if e1 >= len(eloq_low) or o1 >= len(ours_low):
            continue
        peaks = by_transition[transition]
        peaks[0].append(eloq_low[e0:e1].max())
        peaks[1].append(ours_low[o0:o1].max())
        peaks[2].append(eloq_high[e0:e1].max())
        peaks[3].append(ours_high[o0:o1].max())
    print("boundary          n   low-band jump E/O    high-band jump E/O")
    for transition, peaks in sorted(by_transition.items(), key=lambda kv: -len(kv[1][0]))[:16]:
        e_low, o_low, e_high, o_high = (np.median(p) for p in peaks)
        print(f"{transition:16s} {len(peaks[0]):4d}   {e_low:5.1f} {o_low:5.1f}        {e_high:5.1f} {o_high:5.1f}")


if __name__ == "__main__":
    main()
