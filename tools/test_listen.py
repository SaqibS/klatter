from array import array
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import listen


class ListeningTests(unittest.TestCase):
    def test_resampling_preserves_duration_and_level_matching_leaves_headroom(self):
        samples = array("h", [1000, -1000] * 100)
        converted = listen.resample(samples, 1000, 2000)
        self.assertEqual(len(converted), 400)
        matched, _ = listen.match_levels(array("d", [40000, -40000] * 100),
                                         array("d", [50000, -50000] * 100), 1000)
        self.assertEqual(matched[0], matched[1])
        self.assertLessEqual(max(abs(x) for x in matched[0]), 32760)

    def test_levels_match_without_clipping_and_preserve_relative_shape(self):
        a = array("h", [0] * 100 + [1000, -1000] * 100)
        b = array("h", [0] * 100 + [3000, -3000] * 100)
        (left, right), gains = listen.match_levels(a, b, 1000)
        self.assertEqual(left, right)
        self.assertEqual(gains, [1.0, 1 / 3])
        with self.assertRaisesRegex(ValueError, "silent"):
            listen.match_levels(array("h", [0] * 100), b, 1000)

    def test_calibration_handles_integer_negative_settings_and_rejects_unreachable_rates(self):
        class Engine:
            bounds = (-10, 10)
            integer = True

            def render(self, jobs, setting):
                samples = array("h", [1000]) * round(120000 / (100 + 10 * setting))
                for _, path in jobs:
                    listen.save(path, samples, 1000)

        engine = Engine()
        # The lower end must render speech rather than have a zero speaking rate.
        engine.bounds = (-9, 10)
        with tempfile.TemporaryDirectory() as folder:
            setting, rate = listen.calibrate(engine, 150, "one two", Path(folder))
            self.assertEqual(setting, 5)
            self.assertAlmostEqual(rate, 150)
            with self.assertRaisesRegex(ValueError, "outside"):
                listen.calibrate(engine, 300, "one two", Path(folder))

    def test_pairs_balance_order_log_mapping_and_refuse_overwrite(self):
        class Engine:
            native = 220

            def render(self, jobs, setting):
                for _, path in jobs:
                    listen.save(path, array("h", [1000, -1000]) * 500, 1000)

        with tempfile.TemporaryDirectory() as parent:
            folder = Path(parent) / "pairs"
            with patch.object(listen.tts_engines, "parse", return_value=Engine()), \
                    patch.object(listen, "calibrate", return_value=(300, 300)):
                manifest = listen.prepare("one two\n\nthree four", ["a", "b"], folder, 300, 17)
                self.assertEqual(sorted(t["order"][0] for t in manifest["trials"]), [0, 1])
                self.assertTrue(all((folder / t["file"]).is_file() for t in manifest["trials"]))
                starts = {listen.prepare("one two", ["a", "b"], Path(parent) / f"seed-{seed}",
                                         300, seed)["trials"][0]["order"][0] for seed in range(8)}
                self.assertEqual(starts, {0, 1})
                with self.assertRaises(FileExistsError):
                    listen.prepare("one two", ["a", "b"], folder, 300, 17)


if __name__ == "__main__":
    unittest.main()
