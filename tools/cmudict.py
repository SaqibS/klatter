"""Converts CMUdict into the sorted pronunciation table Klatter embeds.

Usage: python tools/cmudict.py data/cmudict/cmudict.dict output.txt

This inspection utility follows Cargo's conversion policy. Cargo generates its
own table from the authoritative raw source; output.txt is not checked in.

Keeps each word's first pronunciation and words spelled only with letters and
apostrophes, uppercases them, and writes unstressed AH as Klatter's schwa AX.
Lines are sorted by byte value so the engine can binary-search them.
"""
import sys

PHONES = set("AA AE AH AO AW AY B CH D DH EH ER EY F G HH IH IY JH K L M N NG OW OY P R S SH T TH UH UW V W Y Z ZH".split())
VOWELS = set("AA AE AH AO AW AY EH ER EY IH IY OW OY UH UW".split())


def convert(phone):
    if phone in PHONES and phone not in VOWELS:
        return phone
    if phone[:-1] in VOWELS and phone[-1:] in ("0", "1", "2"):
        return "AX0" if phone == "AH0" else phone
    raise ValueError(phone)


def main():
    source, target = sys.argv[1:]
    entries = {}
    with open(source, encoding="utf-8") as f:
        for line in f:
            line = line.split("#")[0].strip()
            if not line or line.startswith(";;;"):
                continue
            word, *phones = line.split()
            word = word.upper()
            if "(" in word or not all(c.isascii() and (c.isalpha() or c == "'") for c in word):
                continue
            if not phones:
                raise ValueError(f"no phonemes for {word}")
            entries.setdefault(word, " ".join(convert(p) for p in phones))
    with open(target, "w", encoding="utf-8", newline="\n") as f:
        for word in sorted(entries, key=lambda w: w.encode()):
            f.write(f"{word} {entries[word]}\n")


if __name__ == "__main__":
    main()
