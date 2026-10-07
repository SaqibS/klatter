"""Synthetic regression checks: python -m unittest discover -s tools."""
import contextlib
import csv
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

import voice_metrics as vm

sys.path.insert(0, str(Path(__file__).parent / "eloquence"))
import align
import bjump
import vlevel
import vowels


class PauseTests(unittest.TestCase):
    def test_internal_pause_is_counted_with_or_without_edge_silence(self):
        for leading, trailing in [(0, 0), (20, 0), (0, 20), (20, 20)]:
            speech = np.array([False] * leading + [True] * 20 + [False] * 20
                              + [True] * 20 + [False] * trailing)
            with self.subTest(leading=leading, trailing=trailing):
                stats = vm.pause_stats(speech)
                self.assertAlmostEqual(stats["pauses_per_min"], 150.0)
                self.assertAlmostEqual(stats["pause_mean_ms"], 200.0)
                self.assertAlmostEqual(stats["pause_fraction"], 20 / len(speech))

    def test_short_closures_and_edge_silence_are_not_pauses(self):
        for mask in [[1] * 60, [0] * 20 + [1] * 60 + [0] * 20,
                     [1] * 20 + [0] * 9 + [1] * 20]:
            self.assertEqual(vm.pause_stats(np.array(mask, dtype=bool)),
                             dict(pauses_per_min=0, pause_mean_ms=0, pause_fraction=0))

    def test_multiple_internal_pauses_include_first_and_last(self):
        speech = np.array([1] * 20 + [0] * 10 + [1] * 20 + [0] * 30 + [1] * 20,
                          dtype=bool)
        stats = vm.pause_stats(speech)
        self.assertAlmostEqual(stats["pauses_per_min"], 200.0)
        self.assertAlmostEqual(stats["pause_mean_ms"], 200.0)
        self.assertAlmostEqual(stats["pause_fraction"], 0.4)


class AlignmentTests(unittest.TestCase):
    def test_exact_anchor_boundaries_and_interpolation(self):
        rows = [{"start_ms": "0", "dur_ms": "103"},
                {"start_ms": "103", "dur_ms": "102"},
                {"start_ms": "205", "dur_ms": "795"}]
        spans = align.reference_spans(rows, [(0, 0), (0.103, 0.201), (1, 2)])
        self.assertEqual(spans.shape, (3, 2))
        self.assertEqual(spans[0, 0], 0)
        self.assertAlmostEqual(spans[0, 1], 0.201)
        self.assertAlmostEqual(spans[1, 0], 0.201)
        self.assertAlmostEqual(spans[1, 1], 0.201 + 0.102 / 0.897 * 1.799)
        self.assertEqual(spans[-1, 1], 2)
        np.testing.assert_allclose(spans[:-1, 1], spans[1:, 0])

    def test_identity_alignment_includes_recording_end(self):
        rows = [{"start_ms": "0", "dur_ms": "5"},
                {"start_ms": "5", "dur_ms": "995"}]
        np.testing.assert_allclose(align.reference_spans(rows, [(0, 0), (1, 1)]),
                                   [[0, 0.005], [0.005, 1]])

    def test_analysis_consumers_need_only_the_aligned_segments(self):
        rows = [dict(ph="IY", stress="1", fn="0", final="0", start_ms=str(150 + i * 100),
                     dur_ms="100", f1="250", f2="2100", f3="2790", wi="1", word="bee",
                     eloq_start=str(0.15 + i * 0.1), eloq_end=str(0.25 + i * 0.1))
                for i in range(5)]
        with tempfile.TemporaryDirectory() as directory:
            with (Path(directory) / "align.tsv").open("w", newline="") as f:
                writer = csv.DictWriter(f, fieldnames=list(rows[0]), delimiter="\t")
                writer.writeheader()
                writer.writerows(rows)
            for module in [vowels, vlevel, bjump]:
                with self.subTest(module=module.__name__), contextlib.ExitStack() as stack:
                    stack.enter_context(patch.object(module, "parse_args", return_value=(directory, "candidate.wav", [])))
                    audio = np.sin(np.arange(16000) * 2 * np.pi * 100 / 16000)
                    stack.enter_context(patch.object(module, "load_reference", return_value=(audio, 16000)))
                    stack.enter_context(patch.object(module.vm, "load", return_value=(audio, 16000)))
                    if module is vowels:
                        stack.enter_context(patch.object(module, "formants_at",
                                                         return_value=np.array([250, 2100, 2790])))
                    output = stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                    module.main()
                    self.assertIn("IY" if module is not bjump else "V>V", output.getvalue())


if __name__ == "__main__":
    unittest.main()
