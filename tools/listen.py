"""Rate- and level-matched blind pairs, without recognizer dependencies.

python tools/listen.py passage.txt baseline.exe candidate.exe --out work/listening
Each nonempty paragraph becomes one trial. Use --render-only to prepare clips.
The manifest contains the blinded mapping; do not inspect it before voting.
"""
import argparse
from array import array
import hashlib
import json
import math
from pathlib import Path
import random
import re
import secrets
import sys
import wave

import tts_engines

QUESTION = ("Which version do you prefer overall? First A, then B. "
            "You can choose A, B, no preference, or unsure.")


def load(path):
    with wave.open(str(path), "rb") as wav:
        if wav.getnchannels() != 1 or wav.getsampwidth() != 2:
            raise ValueError(f"{path}: expected mono 16-bit PCM")
        rate = wav.getframerate()
        samples = array("h", wav.readframes(wav.getnframes()))
    if sys.byteorder != "little":
        samples.byteswap()
    return samples, rate


def save(path, samples, rate):
    data = array("h", samples)
    if sys.byteorder != "little":
        data.byteswap()
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes(data.tobytes())


def active_bounds(samples, rate):
    hop = max(1, rate // 100)
    levels = [math.sqrt(sum(x * x for x in samples[i:i + hop]) /
                        len(samples[i:i + hop]))
              for i in range(0, len(samples), hop)]
    peak = max(levels, default=0)
    if peak == 0:
        raise ValueError("silent audio cannot be calibrated")
    active = [i for i, level in enumerate(levels) if level > peak / 100]
    return active[0] * hop, min(len(samples), (active[-1] + 1) * hop)


def measured_wpm(text, samples, rate):
    first, last = active_bounds(samples, rate)
    words = re.findall(r"[a-z]+(?:'[a-z]+)*", text.lower().replace("\u2019", "'"))
    if not words:
        raise ValueError("calibration text contains no words")
    return len(words) * 60 * rate / (last - first)


def resample(samples, source_rate, target_rate):
    if source_rate == target_rate:
        return samples
    try:
        from scipy.signal import resample_poly
    except ImportError as error:
        raise RuntimeError("comparing different sample rates requires scipy") from error
    divisor = math.gcd(source_rate, target_rate)
    return array("d", resample_poly(array("d", samples),
                                   target_rate // divisor, source_rate // divisor))


def calibrate(engine, target, text, folder):
    if not math.isfinite(target) or target <= 0:
        raise ValueError("target rate must be positive and finite")
    path = str(folder / "calibration.wav")
    cache = {}

    def measure(setting):
        if setting not in cache:
            engine.render([(text, path)], setting)
            samples, rate = load(path)
            cache[setting] = measured_wpm(text, samples, rate)
        return cache[setting]

    low, high = engine.bounds
    if not measure(low) <= target <= measure(high):
        raise ValueError(f"target {target:g} WPM is outside this engine's measured range")
    for _ in range(12):
        if engine.integer and high - low <= 1:
            break
        middle = ((low + high) // 2 if engine.integer else
                  math.sqrt(low * high) if low > 0 else (low + high) / 2)
        if measure(middle) < target:
            low = middle
        else:
            high = middle
    setting = min((low, high), key=lambda s: abs(math.log(measure(s) / target)))
    return setting, measure(setting)


def match_levels(a, b, rate):
    def level(samples):
        first, last = active_bounds(samples, rate)
        active = samples[first:last]
        return math.sqrt(sum(x * x for x in active) / len(active))

    levels = [level(a), level(b)]
    target = min(levels)
    gains = [target / rms for rms in levels]
    peak = max(abs(x * gain) for samples, gain in zip((a, b), gains) for x in samples)
    headroom = min(1.0, 32760 / peak)
    gains = [gain * headroom for gain in gains]
    matched = [array("h", (round(x * gain) for x in samples))
               for samples, gain in zip((a, b), gains)]
    return matched, gains


def prepare(text, specs, folder, wpm, seed):
    folder.mkdir(parents=True, exist_ok=False)
    paragraphs = [p.strip() for p in re.split(r"\n\s*\n", text) if p.strip()]
    if not paragraphs:
        raise ValueError("no listening passages")
    rng = random.Random(seed)
    engines, settings = [], []
    for index, spec in enumerate(specs):
        directory = folder / f"engine-{index}"
        directory.mkdir()
        engine = tts_engines.parse(spec)
        setting, actual = calibrate(engine, wpm, paragraphs[0], directory)
        engines.append(engine)
        settings.append({"spec": spec, "setting": setting, "calibration_wpm": actual,
                         "exe_sha256": hashlib.sha256(Path(engine.exe).read_bytes()).hexdigest()
                         if hasattr(engine, "exe") else None})
    prompt = folder / "prompt.wav"
    engines[0].render([(QUESTION, str(prompt))], engines[0].native)
    question, question_rate = load(prompt)
    start = rng.randrange(2)
    orders = [(i + start) % 2 for i in range(len(paragraphs))]
    rng.shuffle(orders)
    trials = []
    for index, (passage, reverse) in enumerate(zip(paragraphs, orders), 1):
        clips, actual_rates, sample_rates = [], [], []
        for engine_index, (engine, configuration) in enumerate(zip(engines, settings)):
            path = folder / f"engine-{engine_index}" / f"trial-{index}.wav"
            engine.render([(passage, str(path))], configuration["setting"])
            samples, sr = load(path)
            clips.append(samples)
            sample_rates.append(sr)
            actual_rates.append(measured_wpm(passage, samples, sr))
        rate = max(question_rate, *sample_rates)
        clips = [resample(samples, sr, rate) for samples, sr in zip(clips, sample_rates)]
        clips, gains = match_levels(*clips, rate)
        order = [1, 0] if reverse else [0, 1]
        silence = array("h", [0]) * rate
        prompt_samples = resample(question, question_rate, rate)
        prompt_peak = max(abs(x) for x in prompt_samples)
        prompt_gain = min(1.0, 32760 / prompt_peak) if prompt_peak else 1.0
        prompt_pcm = array("h", (round(x * prompt_gain) for x in prompt_samples))
        combined = prompt_pcm + silence + clips[order[0]] + silence + clips[order[1]] + silence
        path = folder / f"pair-{index}.wav"
        save(path, combined, rate)
        trials.append({"file": path.name, "text": passage, "order": order,
                       "actual_wpm": actual_rates, "gains": gains, "sample_rate": rate,
                       "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "vote": None})
    manifest = {"seed": seed, "target_wpm": wpm, "engines": settings, "trials": trials}
    (folder / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("text", type=Path)
    parser.add_argument("engines", nargs=2)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--wpm", type=float, default=300)
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument("--render-only", action="store_true")
    args = parser.parse_args()
    manifest = prepare(args.text.read_text(encoding="utf-8-sig"), args.engines,
                       args.out, args.wpm, args.seed if args.seed is not None else secrets.randbits(64))
    if args.render_only:
        for trial in manifest["trials"]:
            print(args.out / trial["file"])
        return
    import winsound
    for index, trial in enumerate(manifest["trials"], 1):
        while True:
            winsound.PlaySound(str(args.out / trial["file"]), winsound.SND_FILENAME)
            vote = input(f"Trial {index}: A, B, same, unsure, or R to replay: ").strip().lower()
            if vote in {"a", "b", "same", "unsure"}:
                trial["vote"] = vote
                (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
                break
            if vote != "r":
                print("Choose A, B, same, unsure, or R.")


if __name__ == "__main__":
    main()
