"""NVDA driver for the Klatter speech synthesizer, which lives in klatter.dll."""
import ctypes
import os
import queue
import threading
from collections import OrderedDict

import config
import nvwave
from autoSettingsUtils.driverSetting import BooleanDriverSetting, DriverSetting, NumericDriverSetting
from autoSettingsUtils.utils import StringParameterInfo
from logHandler import log
from speech.commands import IndexCommand, PitchCommand
from synthDriverHandler import SynthDriver as BaseSynthDriver
from synthDriverHandler import VoiceInfo, synthDoneSpeaking, synthIndexReached

# Klatter's own rate, and the 20 rates Klattara offers. The sample rate is a
# choice among these, not a slider: a slider over 8000-96000 cannot land on
# an exact rate. The engine renders at any of them (each checked clean).
DEFAULT_SAMPLE_RATE = 16000
SAMPLE_RATES = (
    8000, 10000, 11000, 11025, 12000, 16000, 20000, 22000, 22050, 24000,
    32000, 40000, 44000, 44100, 48000, 64000, 80000, 88000, 88200, 96000,
)
SAMPLE_RATE = DEFAULT_SAMPLE_RATE  # kept for speak_items' callers and tests
BYTES_PER_SAMPLE = 2
DLL_PATH = os.path.join(os.path.dirname(__file__), "klatter.dll")

# The exact rate and pitch, set in the Klatter settings panel. Klatter's rate is
# in words per minute already; these are the ends it is checked at.
MIN_WPM, MAX_WPM = 30, 1920
MIN_PITCH_HZ, MAX_PITCH_HZ = 40, 400

# Voice quality, in the order of the engine's quality array (src/quality.rs),
# with NVDA's setting id and label for each. A test checks this against the
# DLL's own klatter_parameter_info. Flutter is off by default.
QUALITY = (
    # code, setting id, label, default, min, max, step
    ("fl", "flutter", "&Flutter", 0, 0, 100, 5),
    ("hs", "headSize", "H&ead size", 100, 64, 135, 1),
    ("rg", "roughness", "&Roughness", 0, 0, 100, 5),
    ("br", "breathiness", "&Breathiness", 0, 0, 72, 2),
    ("oq", "openQuotient", "&Open quotient", 50, 20, 90, 1),
    ("tl", "spectralTilt", "Spectral &tilt", 0, 0, 41, 1),
    ("vvl", "voicingLevel", "&Voicing level", 100, 0, 200, 5),
    ("fvl", "fricationLevel", "Fr&ication level", 100, 0, 200, 5),
    ("avl", "aspirationLevel", "As&piration level", 100, 0, 200, 5),
    ("st", "accentSize", "Accent si&ze", 100, 0, 300, 10),
    ("fa", "baselineFall", "Baseline fa&ll", 8, 0, 40, 1),
    ("f4", "f4", "F&4 in hertz", 3250, 2000, 6000, 50),
    ("b4", "f4Bandwidth", "F4 bandwidth", 200, 50, 2000, 10),
    ("f5", "f5", "F&5 in hertz", 3700, 2500, 6500, 50),
    ("b5", "f5Bandwidth", "F5 bandwidth", 200, 50, 2000, 10),
    ("f6", "f6", "F&6 in hertz", 4990, 3000, 8000, 50),
    ("b6", "f6Bandwidth", "F6 bandwidth", 500, 50, 3000, 10),
)


def feed_bytes(sample_rate):
    """Short pieces let a cancel take effect quickly: a tenth of a second."""
    return sample_rate // 10 * BYTES_PER_SAMPLE


FEED_BYTES = feed_bytes(DEFAULT_SAMPLE_RATE)


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
    quality = ctypes.POINTER(ctypes.c_double)
    dll.klatter_utterance_create_quality.argtypes = (
        dll.klatter_speak.argtypes + [quality, ctypes.c_size_t, ctypes.c_void_p]
    )
    dll.klatter_utterance_create_quality.restype = ctypes.c_void_p
    dll.klatter_parameter_count.argtypes = []
    dll.klatter_parameter_count.restype = ctypes.c_size_t
    dll.klatter_parameter_info.argtypes = [ctypes.c_size_t, ctypes.POINTER(KlatterParameterInfo)]
    dll.klatter_parameter_info.restype = ctypes.c_int
    return dll


class KlatterParameterInfo(ctypes.Structure):
    _fields_ = [("code", ctypes.c_char_p), ("name", ctypes.c_char_p),
                ("unit", ctypes.c_char_p), ("help", ctypes.c_char_p),
                ("min", ctypes.c_double), ("max", ctypes.c_double),
                ("default_value", ctypes.c_double), ("step", ctypes.c_double)]


def parameter_table(dll):
    """The engine's own settings: (code, default, min, max) for each index."""
    table = []
    for index in range(dll.klatter_parameter_count()):
        info = KlatterParameterInfo()
        dll.klatter_parameter_info(index, ctypes.byref(info))
        table.append((info.code.decode(), info.default_value, info.min, info.max))
    return table


def whole(value, low, high):
    """A whole number within range. NVDA's config refuses a float for an
    integer setting, and wx.SpinCtrl refuses one too: Klattara's stuck OK
    button came from exactly that, so every stored value goes through here."""
    return int(min(high, max(low, round(value))))


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


def make_voice(default, rate, pitch, inflection, volume, wpm=None, pitch_hz=None):
    """Maps NVDA's 0-100 settings, where 50 is Klatter's natural voice, onto
    physical units. Rate and pitch are exponential so each step sounds alike.
    An exact words per minute or hertz, when given, replaces the slider's."""
    hz = default.pitch_hz * 2 ** ((pitch - 50) / 50)
    if pitch_hz is not None:
        # A pitch command (as for capitals) still moves an exact pitch by the
        # same ratio as it moves the slider's.
        hz = pitch_hz * 2 ** ((pitch - 50) / 50)
    return KlatterVoice(
        wpm=wpm if wpm is not None else default.wpm * 2 ** ((rate - 50) / 50),
        pitch_hz=hz,
        inflection=default.inflection * inflection / 50,
        volume=volume / 100,
    )


def _quality_setting(code, setting_id, label, default, low, high, step):
    return NumericDriverSetting(
        setting_id, label, defaultVal=default, minVal=low, maxVal=high,
        minStep=step, normalStep=step, largeStep=step * 5,
    )


class SynthDriver(BaseSynthDriver):
    name = "klatter"
    description = "Klatter"
    # The exact rate and pitch are hidden from NVDA's automatic Speech panel
    # (ids starting with an underscore are skipped) and set in the Klatter
    # panel instead, with exact numeric inputs; they are still saved.
    _rateWpmSetting = NumericDriverSetting(
        "_rateWpm", "Rate in &words per minute", defaultVal=219, minVal=MIN_WPM, maxVal=MAX_WPM,
    )
    _rateInWpmSetting = BooleanDriverSetting("_rateInWpm", "Use rate in words per &minute", defaultVal=False)
    _pitchHertzSetting = NumericDriverSetting(
        "_pitchHertz", "Pitch in &hertz", defaultVal=100, minVal=MIN_PITCH_HZ, maxVal=MAX_PITCH_HZ,
    )
    _pitchInHertzSetting = BooleanDriverSetting("_pitchInHertz", "Use pitch in &hertz", defaultVal=False)
    _sampleRateSetting = DriverSetting(
        "sampleRate",
        # Higher rates carry more of the hiss of S and F; lower ones sound duller.
        "Sample &rate",
        defaultVal=str(DEFAULT_SAMPLE_RATE),
    )
    supportedSettings = (
        BaseSynthDriver.RateSetting(),
        _rateWpmSetting,
        _rateInWpmSetting,
        BaseSynthDriver.PitchSetting(),
        _pitchHertzSetting,
        _pitchInHertzSetting,
        BaseSynthDriver.InflectionSetting(),
        BaseSynthDriver.VolumeSetting(),
        _sampleRateSetting,
    ) + tuple(_quality_setting(*q) for q in QUALITY)
    supportedCommands = {IndexCommand, PitchCommand}
    supportedNotifications = {synthIndexReached, synthDoneSpeaking}

    @classmethod
    def check(cls):
        return os.path.isfile(DLL_PATH)

    def __init__(self):
        super().__init__()
        self._rate = self._pitch = self._inflection = 50
        self._volume = 100
        self._rateWpm = self._rateWpmSetting.defaultVal
        self._rateInWpm = False
        self._pitchHertz = self._pitchHertzSetting.defaultVal
        self._pitchInHertz = False
        # True while settings load, so a restored slider does not switch the
        # exact modes off; set by loadSettings.
        self._loadingSettings = False
        self._sampleRate = DEFAULT_SAMPLE_RATE
        self._quality = {q[1]: q[3] for q in QUALITY}
        self._dll = load_engine(DLL_PATH)
        self._default = KlatterVoice()
        self._dll.klatter_default_voice(ctypes.byref(self._default))
        self._player = None
        self._playerRate = None
        self._ensurePlayer()
        # Bumped by cancel; work tagged with an older generation is abandoned.
        self._generation = 0
        self._cancel_lock = threading.Lock()
        self._active_cancel = None
        self._queue = queue.Queue()
        self._thread = threading.Thread(target=self._run, name="klatter", daemon=True)
        self._thread.start()

    def _ensurePlayer(self):
        """Opens the player at the chosen sample rate, or reopens it when the
        rate has changed. The speech thread calls this between utterances."""
        if self._player is not None and self._playerRate == self._sampleRate:
            return
        player = nvwave.WavePlayer(
            channels=1,
            samplesPerSec=self._sampleRate,
            bitsPerSample=BYTES_PER_SAMPLE * 8,
            outputDevice=config.conf["audio"]["outputDevice"],
        )
        old, self._player, self._playerRate = self._player, player, self._sampleRate
        if old is not None:
            old.close()

    def loadSettings(self, onlyChanged=False):
        if not onlyChanged:
            self._loadingSettings = True
        try:
            super().loadSettings(onlyChanged)
        finally:
            self._loadingSettings = False

    def saveSettings(self):
        # NVDA refuses a float or out-of-range value for an integer setting as
        # it saves, which would leave OK and Apply doing nothing.
        self._rateWpm = whole(self._rateWpm, MIN_WPM, MAX_WPM)
        self._pitchHertz = whole(self._pitchHertz, MIN_PITCH_HZ, MAX_PITCH_HZ)
        for code, setting_id, label, default, low, high, step in QUALITY:
            self._quality[setting_id] = whole(self._quality[setting_id], low, high)
        super().saveSettings()

    def _getAvailableVoices(self):
        return OrderedDict(klatter=VoiceInfo("klatter", "Klatter", "en"))

    def _get_voice(self):
        return "klatter"

    # NVDA's dialog builds this name as f"available{id.capitalize()}s", and
    # "sampleRate".capitalize() is "Samplerate": a capital R would make the
    # lookup miss and the combo box never appear (Klattara 1.7 did that).
    def _get_availableSamplerates(self):
        return OrderedDict(
            (str(rate), StringParameterInfo(str(rate), f"{rate} Hz")) for rate in SAMPLE_RATES
        )

    def _get_sampleRate(self):
        return str(self._sampleRate)

    def _set_sampleRate(self, value):
        rate = int(value)
        if rate not in SAMPLE_RATES:
            raise ValueError(f"Unsupported sample rate: {value}")
        self._sampleRate = rate

    # NVDA's base driver discards settings, so each one needs storage here.
    def _get_rate(self):
        return self._rate

    def _set_rate(self, value):
        self._rate = value
        # Moving NVDA's rate slider goes back to its scale, as in Klattara.
        if not self._loadingSettings:
            self._rateInWpm = False

    def _get__rateWpm(self):
        return whole(self._rateWpm, MIN_WPM, MAX_WPM)

    def _set__rateWpm(self, value):
        self._rateWpm = whole(value, MIN_WPM, MAX_WPM)

    def _get__rateInWpm(self):
        return self._rateInWpm

    def _set__rateInWpm(self, value):
        self._rateInWpm = bool(value)

    def configure_rate_wpm(self, enabled, wpm):
        """Called by the Klatter settings panel."""
        self._rateWpm = whole(wpm, MIN_WPM, MAX_WPM)
        self._rateInWpm = bool(enabled)

    def _get_pitch(self):
        return self._pitch

    def _set_pitch(self, value):
        self._pitch = value
        if not self._loadingSettings:
            self._pitchInHertz = False

    def _get__pitchHertz(self):
        return whole(self._pitchHertz, MIN_PITCH_HZ, MAX_PITCH_HZ)

    def _set__pitchHertz(self, value):
        self._pitchHertz = whole(value, MIN_PITCH_HZ, MAX_PITCH_HZ)

    def _get__pitchInHertz(self):
        return self._pitchInHertz

    def _set__pitchInHertz(self, value):
        self._pitchInHertz = bool(value)

    def configure_pitch_hertz(self, enabled, hertz):
        """Called by the Klatter settings panel."""
        self._pitchHertz = whole(hertz, MIN_PITCH_HZ, MAX_PITCH_HZ)
        self._pitchInHertz = bool(enabled)

    def _get_inflection(self):
        return self._inflection

    def _set_inflection(self, value):
        self._inflection = value

    def _get_volume(self):
        return self._volume

    def _set_volume(self, value):
        self._volume = value

    def _quality_array(self):
        values = [float(self._quality[q[1]]) for q in QUALITY]
        return (ctypes.c_double * len(values))(*values)

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
        quality = self._quality_array()
        with self._cancel_lock:
            self._queue.put((self._generation, self._sampleRate, quality, utterances))

    def _voice(self, pitch):
        return make_voice(
            self._default, self.rate, pitch, self.inflection, self.volume,
            wpm=self._rateWpm if self._rateInWpm else None,
            pitch_hz=self._pitchHertz if self._pitchInHertz else None,
        )

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
            generation, sample_rate, quality, utterances = task
            try:
                if generation == self._generation and sample_rate != self._playerRate:
                    self._ensurePlayer()
                for voice, items in utterances:
                    if not self._play(generation, voice, items, sample_rate, quality):
                        break
            except Exception:
                log.error("Klatter could not speak", exc_info=True)
            if generation == self._generation and self._queue.empty():
                self._player.idle()
                if generation == self._generation and self._queue.empty():
                    synthDoneSpeaking.notify(synth=self)

    def _play(self, generation, voice, items, sample_rate=None, quality=None):
        """Returns False once this generation is cancelled."""
        if generation != self._generation:
            return False
        sample_rate = sample_rate or self._sampleRate
        quality = quality if quality is not None else self._quality_array()
        texts, marks = encode_items(items)
        utterance = self._dll.klatter_utterance_create_quality(
            texts, marks, len(items), sample_rate, ctypes.byref(voice),
            quality, len(quality), None,
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
            feed = feed_bytes(sample_rate)
            output = (ctypes.c_int16 * (feed // BYTES_PER_SAMPLE))()
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
                if not self._feed(generation, audio, offsets, feed):
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

    def _feed(self, generation, audio, marks, feed=FEED_BYTES):
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
            for k in range(start, max(start + 1, end), feed):
                if generation != self._generation:
                    return False
                piece = audio[k:min(k + feed, end)]
                last = k + feed >= end
                if piece or (last and onDone):
                    self._player.feed(piece, onDone=onDone if last else None)
                if generation != self._generation:
                    # Cancel may have stopped the player just before this feed.
                    self._player.stop()
                    return False
            start = end
        return True


def _add_quality_properties():
    """A getter and setter for each voice-quality setting, as NVDA's
    AutoPropertyObject looks for (_get_flutter, _set_flutter, ...)."""
    for code, setting_id, label, default, low, high, step in QUALITY:
        def getter(self, setting_id=setting_id, low=low, high=high):
            return whole(self._quality[setting_id], low, high)

        def setter(self, value, setting_id=setting_id, low=low, high=high):
            self._quality[setting_id] = whole(value, low, high)

        setattr(SynthDriver, f"_get_{setting_id}", getter)
        setattr(SynthDriver, f"_set_{setting_id}", setter)


_add_quality_properties()
