"""NVDA driver for the Klatter speech synthesizer, which lives in klatter.dll."""
import ctypes
import os
import queue
import threading
from collections import OrderedDict

import config
import nvwave
from logHandler import log
from speech.commands import IndexCommand, PitchCommand
from synthDriverHandler import SynthDriver as BaseSynthDriver
from synthDriverHandler import VoiceInfo, synthDoneSpeaking, synthIndexReached

SAMPLE_RATE = 16000
BYTES_PER_SAMPLE = 2
# Short pieces let a cancel take effect quickly even during a long utterance.
FEED_BYTES = SAMPLE_RATE // 10 * BYTES_PER_SAMPLE
DLL_PATH = os.path.join(os.path.dirname(__file__), "klatter.dll")


class KlatterVoice(ctypes.Structure):
    _fields_ = [("wpm", ctypes.c_double), ("pitch_hz", ctypes.c_double),
                ("inflection", ctypes.c_double), ("volume", ctypes.c_double)]


class KlatterSpeech(ctypes.Structure):
    _fields_ = [("samples", ctypes.POINTER(ctypes.c_int16)), ("sample_count", ctypes.c_size_t),
                ("mark_ids", ctypes.POINTER(ctypes.c_uint32)),
                ("mark_samples", ctypes.POINTER(ctypes.c_size_t)), ("mark_count", ctypes.c_size_t)]


class KlatterWord(ctypes.Structure):
    _fields_ = [("input_item", ctypes.c_size_t), ("byte_offset", ctypes.c_size_t),
                ("byte_length", ctypes.c_size_t), ("sample", ctypes.c_size_t)]


class KlatterPullBlock(ctypes.Structure):
    _fields_ = [("sample_count", ctypes.c_size_t),
                ("mark_ids", ctypes.POINTER(ctypes.c_uint32)),
                ("mark_samples", ctypes.POINTER(ctypes.c_size_t)), ("mark_count", ctypes.c_size_t),
                ("words", ctypes.POINTER(KlatterWord)), ("word_count", ctypes.c_size_t)]


def load_engine(path):
    dll = ctypes.CDLL(path)
    dll.klatter_default_voice.argtypes = [ctypes.POINTER(KlatterVoice)]
    dll.klatter_default_voice.restype = None
    dll.klatter_speak.argtypes = [ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint32),
                                  ctypes.c_size_t, ctypes.c_uint32, ctypes.POINTER(KlatterVoice)]
    dll.klatter_speak.restype = ctypes.POINTER(KlatterSpeech)
    dll.klatter_free.argtypes = [ctypes.POINTER(KlatterSpeech)]
    dll.klatter_free.restype = None
    dll.klatter_utterance_create.argtypes = dll.klatter_speak.argtypes + [ctypes.c_void_p]
    dll.klatter_utterance_create.restype = ctypes.c_void_p
    dll.klatter_utterance_read.argtypes = [
        ctypes.c_void_p, ctypes.POINTER(ctypes.c_int16), ctypes.c_size_t,
        ctypes.POINTER(KlatterPullBlock),
    ]
    dll.klatter_utterance_read.restype = ctypes.c_int
    dll.klatter_utterance_free.argtypes = [ctypes.c_void_p]
    dll.klatter_utterance_free.restype = None
    dll.klatter_utterance_cancel_handle.argtypes = [ctypes.c_void_p]
    dll.klatter_utterance_cancel_handle.restype = ctypes.c_void_p
    dll.klatter_cancel.argtypes = [ctypes.c_void_p]
    dll.klatter_cancel.restype = None
    dll.klatter_cancel_free.argtypes = [ctypes.c_void_p]
    dll.klatter_cancel_free.restype = None
    return dll


def encode_items(items):
    count = len(items)
    texts = (ctypes.c_char_p * count)(*(None if isinstance(i, int) else i.encode("utf-8") for i in items))
    marks = (ctypes.c_uint32 * count)(*(i if isinstance(i, int) else 0 for i in items))
    return texts, marks


def speech_data(s):
    audio = ctypes.string_at(s.samples, s.sample_count * BYTES_PER_SAMPLE) if s.sample_count else b""
    offsets = [(s.mark_ids[k], s.mark_samples[k] * BYTES_PER_SAMPLE) for k in range(s.mark_count)]
    return audio, offsets


def speak_items(dll, items, voice):
    """Speaks strings and integer marks as one utterance; returns PCM bytes and
    (mark, byte offset) pairs."""
    texts, marks = encode_items(items)
    result = dll.klatter_speak(texts, marks, len(items), SAMPLE_RATE, ctypes.byref(voice))
    if not result:
        raise ValueError("Klatter rejected the input")
    try:
        return speech_data(result.contents)
    finally:
        dll.klatter_free(result)


def make_voice(default, rate, pitch, inflection, volume):
    """Maps NVDA's 0-100 settings, where 50 is Klatter's natural voice, onto
    physical units. Rate and pitch are exponential so each step sounds alike."""
    return KlatterVoice(
        wpm=default.wpm * 2 ** ((rate - 50) / 50),
        pitch_hz=default.pitch_hz * 2 ** ((pitch - 50) / 50),
        inflection=default.inflection * inflection / 50,
        volume=volume / 100,
    )


class SynthDriver(BaseSynthDriver):
    name = "klatter"
    description = "Klatter"
    supportedSettings = (
        BaseSynthDriver.RateSetting(),
        BaseSynthDriver.PitchSetting(),
        BaseSynthDriver.InflectionSetting(),
        BaseSynthDriver.VolumeSetting(),
    )
    supportedCommands = {IndexCommand, PitchCommand}
    supportedNotifications = {synthIndexReached, synthDoneSpeaking}

    @classmethod
    def check(cls):
        return os.path.isfile(DLL_PATH)

    def __init__(self):
        super().__init__()
        self._rate = self._pitch = self._inflection = 50
        self._volume = 100
        self._dll = load_engine(DLL_PATH)
        self._default = KlatterVoice()
        self._dll.klatter_default_voice(ctypes.byref(self._default))
        self._player = nvwave.WavePlayer(
            channels=1,
            samplesPerSec=SAMPLE_RATE,
            bitsPerSample=BYTES_PER_SAMPLE * 8,
            outputDevice=config.conf["audio"]["outputDevice"],
        )
        # Bumped by cancel; work tagged with an older generation is abandoned.
        self._generation = 0
        self._cancel_lock = threading.Lock()
        self._active_cancel = None
        self._queue = queue.Queue()
        self._thread = threading.Thread(target=self._run, name="klatter", daemon=True)
        self._thread.start()

    def _getAvailableVoices(self):
        return OrderedDict(klatter=VoiceInfo("klatter", "Klatter", "en"))

    def _get_voice(self):
        return "klatter"

    # NVDA's base driver discards settings, so each one needs storage here.
    def _get_rate(self):
        return self._rate

    def _set_rate(self, value):
        self._rate = value

    def _get_pitch(self):
        return self._pitch

    def _set_pitch(self, value):
        self._pitch = value

    def _get_inflection(self):
        return self._inflection

    def _set_inflection(self, value):
        self._inflection = value

    def _get_volume(self):
        return self._volume

    def _set_volume(self, value):
        self._volume = value

    def speak(self, speechSequence):
        # A pitch change splits the sequence because the engine takes one voice per utterance.
        utterances = []
        items = []
        pitch = self.pitch
        for item in speechSequence:
            if isinstance(item, str):
                items.append(item)
            elif isinstance(item, IndexCommand):
                items.append(item.index)
            elif isinstance(item, PitchCommand):
                if items:
                    utterances.append((self._voice(pitch), items))
                    items = []
                pitch = item.newValue
        if items:
            utterances.append((self._voice(pitch), items))
        with self._cancel_lock:
            self._queue.put((self._generation, utterances))

    def _voice(self, pitch):
        return make_voice(self._default, self.rate, pitch, self.inflection, self.volume)

    def cancel(self):
        # The lock protects the native handle from being freed while cancellation uses it.
        with self._cancel_lock:
            self._generation += 1
            if self._active_cancel is not None:
                self._dll.klatter_cancel(self._active_cancel)
            while True:
                try:
                    self._queue.get_nowait()
                except queue.Empty:
                    break
        self._player.stop()

    def pause(self, switch):
        self._player.pause(switch)

    def terminate(self):
        self.cancel()
        self._queue.put(None)
        self._thread.join()
        self._player.close()
        super().terminate()

    def _run(self):
        while (task := self._queue.get()) is not None:
            generation, utterances = task
            try:
                for voice, items in utterances:
                    if not self._play(generation, voice, items):
                        break
            except Exception:
                log.error("Klatter could not speak", exc_info=True)
            if generation == self._generation and self._queue.empty():
                self._player.idle()
                if generation == self._generation and self._queue.empty():
                    synthDoneSpeaking.notify(synth=self)

    def _play(self, generation, voice, items):
        """Returns False once this generation is cancelled."""
        if generation != self._generation:
            return False
        texts, marks = encode_items(items)
        utterance = self._dll.klatter_utterance_create(
            texts, marks, len(items), SAMPLE_RATE, ctypes.byref(voice), None,
        )
        if not utterance:
            raise ValueError("Klatter rejected the input")
        cancel = None
        try:
            cancel = self._dll.klatter_utterance_cancel_handle(utterance)
            if not cancel:
                raise RuntimeError("Klatter could not create a cancellation handle")
            with self._cancel_lock:
                if generation != self._generation:
                    return False
                self._active_cancel = cancel
            output = (ctypes.c_int16 * (FEED_BYTES // BYTES_PER_SAMPLE))()
            block = KlatterPullBlock()
            position = 0
            while generation == self._generation:
                status = self._dll.klatter_utterance_read(
                    utterance, output, len(output), ctypes.byref(block),
                )
                if status == -2:
                    return False
                if status < 0:
                    raise RuntimeError("Klatter could not synthesize the utterance")
                audio = ctypes.string_at(output, block.sample_count * BYTES_PER_SAMPLE)
                offsets = [
                    (block.mark_ids[k], (block.mark_samples[k] - position) * BYTES_PER_SAMPLE)
                    for k in range(block.mark_count)
                ]
                if not self._feed(generation, audio, offsets):
                    return False
                position += block.sample_count
                if status == 0:
                    return True
            return False
        finally:
            with self._cancel_lock:
                if self._active_cancel == cancel:
                    self._active_cancel = None
                if cancel is not None:
                    self._dll.klatter_cancel_free(cancel)
            self._dll.klatter_utterance_free(utterance)

    def _feed(self, generation, audio, marks):
        if generation != self._generation:
            return False
        start = 0
        for mark, end in marks + [(None, len(audio))]:
            onDone = None
            if mark is not None:
                def onDone(mark=mark):
                    if generation == self._generation:
                        synthIndexReached.notify(synth=self, index=mark)
            # Empty pieces still carry their mark, as NVDA's eSpeak driver does.
            for k in range(start, max(start + 1, end), FEED_BYTES):
                if generation != self._generation:
                    return False
                piece = audio[k:min(k + FEED_BYTES, end)]
                last = k + FEED_BYTES >= end
                if piece or (last and onDone):
                    self._player.feed(piece, onDone=onDone if last else None)
                if generation != self._generation:
                    # Cancel may have stopped the player just before this feed.
                    self._player.stop()
                    return False
            start = end
        return True
