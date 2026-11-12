"""Regression checks for the embedded dictionary converter."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import cmudict


class CmudictTests(unittest.TestCase):
    def test_conversion_keeps_first_reading_normalizes_and_sorts(self):
        source = """;;; dictionary header
rabbit R AE1 B AH0 T # comment
read R EH1 D
read(2) R IY1 D
read R IY1 D
can't K AE1 N T
alpha AE1 L F AH0
word-2 W ER1 D
"""
        with tempfile.TemporaryDirectory() as directory:
            src = Path(directory) / "cmudict.dict"
            out = Path(directory) / "cmudict.txt"
            src.write_text(source, encoding="utf-8")
            with patch.object(sys, "argv", ["cmudict.py", str(src), str(out)]):
                cmudict.main()
            self.assertEqual(out.read_text(encoding="utf-8"),
                             "ALPHA AE1 L F AX0\nCAN'T K AE1 N T\nRABBIT R AE1 B AX0 T\nREAD R EH1 D\n")

    def test_malformed_phones_are_rejected(self):
        for phone in ["UNKNOWN1", "B1", "AA21", "AH9", "AE", "", "AX0"]:
            with self.subTest(phone=phone), self.assertRaises(ValueError):
                cmudict.convert(phone)

    def test_entries_without_phonemes_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            src = Path(directory) / "cmudict.dict"
            out = Path(directory) / "cmudict.txt"
            src.write_text("empty\n", encoding="utf-8")
            with patch.object(sys, "argv", ["cmudict.py", str(src), str(out)]):
                with self.assertRaisesRegex(ValueError, "no phonemes for EMPTY"):
                    cmudict.main()
            self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main()
