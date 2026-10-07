"""Runs the NVDA driver against klatter.dll with NVDA's modules replaced by stubs.

Usage: python nvda/test_driver.py path/to/klatter.dll
The DLL must match this Python's architecture.
"""
import os
import sys
import threading
import types
import unittest

events = []


class Notification:
    def __init__(self, name):
        self.name = name

    def notify(self, **kwargs):
        events.append((self.name, kwargs.get("index")))


class WavePlayer:
    """Plays instantly: a chunk's callback fires as soon as it is fed."""

    def __init__(self, channels, samplesPerSec, bitsPerSample, outputDevice):
        assert (channels, samplesPerSec, bitsPerSample) == (1, 16000, 16)
        self.fed = bytearray()
        self.stops = 0
        self.on_feed = None

    def feed(self, data, onDone=None):
        self.fed += data
        events.append(("audio", len(data)))
        if self.on_feed:
            self.on_feed()
        if onDone:
            onDone()

    def idle(self):
        pass

    def stop(self):
        self.stops += 1

    def pause(self, switch):
        pass

    def close(self):
        pass


class BaseSynthDriver:
    RateSetting = PitchSetting = InflectionSetting = VolumeSetting = classmethod(lambda cls: None)

    # Like NVDA's, these route to _get_ and _set_ methods that by default store nothing.
    rate = property(lambda self: self._get_rate(), lambda self, v: self._set_rate(v))
    pitch = property(lambda self: self._get_pitch(), lambda self, v: self._set_pitch(v))
    inflection = property(lambda self: self._get_inflection(), lambda self, v: self._set_inflection(v))
    volume = property(lambda self: self._get_volume(), lambda self, v: self._set_volume(v))

    def _get_rate(self):
        return 0

    def _set_rate(self, value):
        pass

    _get_pitch = _get_inflection = _get_volume = _get_rate
    _set_pitch = _set_inflection = _set_volume = _set_rate

    def terminate(self):
        pass


class Command:
    def __init__(self, value):
        self.value = value


class IndexCommand(Command):
    @property
    def index(self):
        return self.value


class PitchCommand(Command):
    @property
    def newValue(self):
        return self.value


def install_stubs():
    def module(name, **attrs):
        m = types.ModuleType(name)
        m.__dict__.update(attrs)
        sys.modules[name] = m

    module("config", conf={"audio": {"outputDevice": "default"}})
    module("nvwave", WavePlayer=WavePlayer)
    module("logHandler", log=types.SimpleNamespace(error=lambda *a, **k: events.append(("error", a))))
    module("speech")
    module("speech.commands", IndexCommand=IndexCommand, PitchCommand=PitchCommand)
    module("synthDriverHandler", SynthDriver=BaseSynthDriver, VoiceInfo=lambda *a: a,
           synthIndexReached=Notification("index"), synthDoneSpeaking=Notification("done"))


install_stubs()
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "addon", "synthDrivers"))
import klatter  # noqa: E402

klatter.DLL_PATH = os.path.abspath(sys.argv.pop(1)) if len(sys.argv) > 1 else klatter.DLL_PATH


class DriverTest(unittest.TestCase):
    def setUp(self):
        events.clear()
        self.synth = klatter.SynthDriver()
        self.synth.rate = self.synth.pitch = self.synth.inflection = 50
        self.synth.volume = 100

    def tearDown(self):
        self.synth.terminate()

    def wait(self):
        done = threading.Event()
        self.synth._queue.put((self.synth._generation, []))
        while not any(e[0] == "done" for e in events):
            done.wait(0.01)

    def test_indexes_follow_their_audio_in_order(self):
        self.synth.speak([IndexCommand(1), "Hello there.", IndexCommand(2), "How are you?", IndexCommand(3)])
        self.wait()
        notices = [e for e in events if e[0] != "audio"]
        self.assertEqual(notices[:3], [("index", 1), ("index", 2), ("index", 3)])
        self.assertEqual(notices[-1], ("done", None))
        first = events.index(("index", 1))
        second = events.index(("index", 2))
        self.assertTrue(all(e[0] != "audio" or e[1] == 0 for e in events[:first]))
        self.assertGreater(sum(e[1] for e in events[first:second] if e[0] == "audio"), 16000)
        audio, _ = klatter.speak_items(self.synth._dll, ["Hello there.", "How are you?"], self.synth._voice(50))
        self.assertEqual(bytes(self.synth._player.fed), audio)

    def test_natural_settings_match_the_engine_default(self):
        v = self.synth._voice(50)
        d = self.synth._default
        self.assertEqual((v.wpm, v.pitch_hz, v.inflection, v.volume), (d.wpm, d.pitch_hz, d.inflection, 1.0))
        self.synth.rate = 100
        self.assertAlmostEqual(self.synth._voice(50).wpm, 2 * d.wpm)

    def test_pitch_command_changes_pitch_for_the_following_text(self):
        self.synth.speak(["cap", PitchCommand(80), "A", PitchCommand(50), IndexCommand(9)])
        self.wait()
        low, _ = klatter.speak_items(self.synth._dll, ["cap"], self.synth._voice(50))
        high, _ = klatter.speak_items(self.synth._dll, ["A"], self.synth._voice(80))
        self.assertEqual(bytes(self.synth._player.fed), low + high)
        self.assertIn(("index", 9), events)

    def test_cancel_abandons_queued_and_playing_speech(self):
        cancelled = threading.Event()

        def cancel_once():
            if not cancelled.is_set():
                self.synth.cancel()
                cancelled.set()

        self.synth._player.on_feed = cancel_once
        self.synth.speak(["A long sentence that needs more than one piece of audio.", IndexCommand(1)])
        self.synth.speak(["Queued speech.", IndexCommand(2)])
        cancelled.wait(5)
        self.synth.speak(["After.", IndexCommand(3)])
        self.wait()
        self.assertEqual([e for e in events if e[0] == "index"], [("index", 3)])
        self.assertGreaterEqual(self.synth._player.stops, 2)
        after, _ = klatter.speak_items(self.synth._dll, ["After."], self.synth._voice(50))
        self.assertEqual(bytes(self.synth._player.fed[-len(after):]), after)
        self.assertEqual(len(self.synth._player.fed), klatter.FEED_BYTES + len(after))

    def test_rejected_input_is_logged_not_raised(self):
        self.synth.speak(["fine"])
        self.synth.rate = float("nan")
        self.synth.speak(["broken"])
        self.wait()
        self.assertTrue(any(e[0] == "error" for e in events))


if __name__ == "__main__":
    unittest.main()
