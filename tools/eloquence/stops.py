"""Stop closures: how deep and how long the silence is, Eloquence vs ours.

For stressed P T K B D G, finds the minimum 10 ms energy (dB re the file's
95th-percentile frame) and how long the level stays within 6 dB of it. Deep,
long closures sound crisp; shallow ones sound mushy. The Eloquence span is
widened by 40 ms each side to absorb alignment error; ours uses the exact span.

Usage: python stops.py WORKDIR [candidate.wav]
"""
import numpy as np

from common import eloq_span, load_align, load_reference, our_span, parse_args, vm

HOP = 0.0025
STOPS = "P T K B D G".split()


def energy_db(x, sr):
    win, hop = int(0.01 * sr), int(HOP * sr)
    frames = np.lib.stride_tricks.sliding_window_view(x, win)[::hop]
    return 10 * np.log10(np.mean(frames ** 2, 1) + 1e-12)


def depth_and_quiet_ms(levels):
    deepest = levels.min()
    return deepest, (levels < deepest + 6).sum() * HOP * 1000


def main():
    workdir, candidate, _ = parse_args(__doc__)
    ref_x, ref_sr = load_reference(workdir)
    ours_x, ours_sr = vm.load(candidate)
    eloq_db, ours_db = energy_db(ref_x, ref_sr), energy_db(ours_x, ours_sr)
    eloq_peak, ours_peak = np.percentile(eloq_db, 95), np.percentile(ours_db, 95)
    rows = load_align(workdir)
    for stop in STOPS:
        eloq, ours = [], []
        for row in rows:
            if row["ph"] != stop or row["stress"] == "0":
                continue
            e0, e1 = eloq_span(row)
            o0, o1 = our_span(row)
            eloq.append(depth_and_quiet_ms(eloq_db[int((e0 - 0.04) / HOP):int((e1 + 0.04) / HOP)] - eloq_peak))
            ours.append(depth_and_quiet_ms(ours_db[int(o0 / HOP):int(o1 / HOP)] - ours_peak))
        eloq, ours = np.array(eloq), np.array(ours)
        print(f"{stop}: n={len(eloq)} Eloquence min dB {np.median(eloq[:, 0]):.0f}, quiet ms {np.median(eloq[:, 1]):.0f}"
              f" | ours min {np.median(ours[:, 0]):.0f}, quiet ms {np.median(ours[:, 1]):.0f}")
    print("Eloquence noise floor dB", round(float(np.percentile(eloq_db, 2) - eloq_peak)))


if __name__ == "__main__":
    main()
