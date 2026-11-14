"""Runs the NVDA driver against klatter.dll with NVDA's modules replaced by stubs.

Usage: python nvda/test_driver.py path/to/klatter.dll
The DLL must match this Python's architecture.
"""
import os
import sys
import threading
import time
import types
import unittest
from unittest.mock import patch

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
        deadline = time.monotonic() + 5
        while not any(e[0] == "done" for e in events):
            self.assertLess(time.monotonic(), deadline, f"Speech did not finish: {events[-10:]}")
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

    def test_mixed_case_identifiers_preserve_playback_indexes(self):
        self.synth.speak([
            IndexCommand(1), "getXMLValue2", IndexCommand(2), "NoteBook", IndexCommand(3),
        ])
        self.wait()
        expected, _ = klatter.speak_items(
            self.synth._dll, ["get XML Value 2", "notebook"], self.synth._voice(50),
        )
        self.assertEqual(bytes(self.synth._player.fed), expected)
        self.assertEqual([e for e in events if e[0] == "index"],
                         [("index", 1), ("index", 2), ("index", 3)])

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

    def test_stale_requests_do_not_enter_native_synthesis(self):
        generation = self.synth._generation
        self.synth.cancel()
        with patch.object(self.synth._dll, "klatter_utterance_create") as create:
            self.assertFalse(self.synth._play(generation, self.synth._voice(50), ["Stale."]))
            create.assert_not_called()

    def test_cancel_during_creation_prevents_the_first_read(self):
        generation = self.synth._generation
        create = self.synth._dll.klatter_utterance_create

        def cancelled_create(*args):
            utterance = create(*args)
            self.synth.cancel()
            return utterance

        with patch.object(self.synth._dll, "klatter_utterance_create", cancelled_create):
            with patch.object(self.synth._dll, "klatter_utterance_read") as read:
                self.assertFalse(self.synth._play(generation, self.synth._voice(50), ["Stale."]))
                read.assert_not_called()
        self.assertIsNone(self.synth._active_cancel)

    def test_cancel_signals_the_native_handle_before_audio_is_returned(self):
        entered = threading.Event()
        release = threading.Event()
        statuses = []
        read = self.synth._dll.klatter_utterance_read

        def blocked_read(*args):
            entered.set()
            if not release.wait(5):
                raise TimeoutError("Test did not release the native read")
            status = read(*args)
            statuses.append(status)
            return status

        with patch.object(self.synth._dll, "klatter_utterance_read", blocked_read):
            self.synth.speak(["Cancelled before audio.", IndexCommand(1)])
            try:
                self.assertTrue(entered.wait(5))
                self.synth.cancel()
            finally:
                release.set()
            self.synth.speak(["Replacement.", IndexCommand(2)])
            self.wait()
        self.assertEqual(statuses[0], -2)
        self.assertEqual([e for e in events if e[0] == "index"], [("index", 2)])
        self.assertIsNone(self.synth._active_cancel)

    def test_failed_reads_release_native_handles(self):
        dll = self.synth._dll
        with patch.object(dll, "klatter_utterance_read", return_value=-1):
            with patch.object(dll, "klatter_cancel_free", wraps=dll.klatter_cancel_free) as cancel_free:
                with patch.object(dll, "klatter_utterance_free", wraps=dll.klatter_utterance_free) as free:
                    with self.assertRaisesRegex(RuntimeError, "could not synthesize"):
                        self.synth._play(self.synth._generation, self.synth._voice(50), ["Hello."])
                    cancel_free.assert_called_once()
                    free.assert_called_once()
        self.assertIsNone(self.synth._active_cancel)


if __name__ == "__main__":
    unittest.main()
