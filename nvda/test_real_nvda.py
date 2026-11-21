"""Starts the Klatter driver the way NVDA does, on NVDA's own code: its
synthDriverHandler, autoSettingsUtils, driverSetting and speech.commands, with
stand-ins only for audio, logging and the configuration store. This catches
what the stub-based test_driver.py cannot: NVDA making _get_/_set_ methods into
properties, creating and saving the synth's settings on first start, loading
them on the next, and the settings dialog and ring reading every setting.

Usage: python3 nvda/test_real_nvda.py path/to/klatter.dll path/to/nvda/source
"""
import builtins
import ctypes
import importlib.util
import sys
import threading
import time
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DRIVER = ROOT / "nvda" / "addon" / "synthDrivers" / "klatter" / "__init__.py"
DLL = Path(sys.argv[1]).resolve()
NVDA_SOURCE = Path(sys.argv[2]).resolve()
events = []
failures = []


def check(ok, message):
    print(("ok: " if ok else "FAIL: ") + message)
    if not ok:
        failures.append(message)


def module(name, **attributes):
    stub = types.ModuleType(name)
    stub.__dict__.update(attributes)
    sys.modules[name] = stub
    return stub


class Log:
    def error(self, message, *args, **kwargs):
        events.append(("error", message, kwargs.get("exc_info")))

    exception = error

    def debug(self, *args, **kwargs):
        pass

    info = warning = io = debugWarning = debug


class WavePlayer:
    def __init__(self, channels, samplesPerSec, bitsPerSample, outputDevice="default", wantDucking=True, purpose=None):
        assert (channels, bitsPerSample) == (1, 16)
        self.rate = samplesPerSec
        self.bytes = 0

    def feed(self, data, size=None, onDone=None):
        self.bytes += size if size is not None else len(data)
        if onDone:
            onDone()

    def idle(self):
        pass

    def stop(self):
        pass

    def pause(self, switch):
        pass

    def close(self):
        pass


class Section(dict):
    """A configuration section, as NVDA's AggregatedSection behaves for this:
    a spec, isSet, and validation of each value against its setting's spec."""

    def __init__(self):
        super().__init__()
        self.spec = {}

    def isSet(self, key):
        return key in self

    def __missing__(self, key):
        # As NVDA's configuration does: a setting never saved reads as its spec's default.
        spec = self.spec.get(key)
        if isinstance(spec, str) and "default=" in spec:
            default = spec.split("default=", 1)[1].split(",")[0].rstrip(")").strip().strip('"')
            return validate(spec, default) if not spec.startswith("string") else default
        raise KeyError(key)

    def __setitem__(self, key, value):
        if isinstance(value, dict) and not isinstance(value, Section):
            section = Section()
            section.update(value)
            value = section
        spec = self.spec.get(key)
        if isinstance(spec, str):
            value = validate(spec, value)
        super().__setitem__(key, value)


def validate(spec, value):
    try:
        from configobj.validate import Validator
    except ImportError:
        pass
    else:
        return Validator().check(spec, value)
    kind, _, args = spec.partition("(")
    params = dict(a.strip().split("=", 1) for a in args.rstrip(")").split(",") if "=" in a)
    if kind == "integer":
        if not isinstance(value, (int, str)) or isinstance(value, bool):
            raise ValueError(f'the value "{value!r}" is of the wrong type')
        value = int(value)
        if "min" in params and value < int(params["min"]):
            raise ValueError(f'the value "{value}" is too small')
        if "max" in params and value > int(params["max"]):
            raise ValueError(f'the value "{value}" is too big')
    elif kind == "boolean" and not isinstance(value, (bool, int, str)):
        raise ValueError(f'the value "{value!r}" is of the wrong type')
    elif kind == "string" and not isinstance(value, str) and value is not None:
        raise ValueError(f'the value "{value!r}" is of the wrong type')
    return value


def load_nvda():
    sys.path.insert(0, str(NVDA_SOURCE))
    builtins._ = lambda text: text
    builtins.pgettext = lambda context, text: text
    module("logHandler", log=Log())
    module("NVDAState", _allowDeprecatedAPI=lambda: True)
    import extensionPoints

    speech = Section()
    speech.spec = {"__many__": {}}
    conf = {"audio": {"outputDevice": "default"}, "speech": speech,
            "debugLog": {"garbageHandler": False, "synthDriver": False}}
    config = module("config", conf=conf, confspec={"speech": {"__many__": {}}},
                    pre_configSave=extensionPoints.Action(), post_configProfileSwitch=extensionPoints.Action())
    module("globalVars", settingsRing=None)
    module("synthSettingsRing", SynthSettingsRing=lambda synth: None)
    module("languageHandler", getLanguageDescription=lambda language: language)
    module("speechDictHandler", loadVoiceDict=lambda synth: None)
    module("synthDrivers", __path__=[])
    module("nvwave", WavePlayer=WavePlayer)
    module("addonHandler", initTranslation=lambda: None)
    module("speech", __path__=[str(NVDA_SOURCE / "speech")])
    import speech.commands  # noqa: F401
    import synthDriverHandler

    return config, synthDriverHandler


def main():
    config, synthDriverHandler = load_nvda()
    spec = importlib.util.spec_from_file_location("klatter_driver", DRIVER)
    driver = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(driver)
    driver.DLL_PATH = str(DLL)
    check(driver.SynthDriver.check(), "check() finds the DLL")

    # First start: NVDA makes the driver, then initSettings creates and saves
    # the settings (synthDriverHandler.getSynthInstance).
    synth = driver.SynthDriver()
    synth.initSettings()
    section = config.conf["speech"]["klatter"]
    check(all(s.id in section for s in synth.supportedSettings), "first start saves every setting")
    for s in synth.supportedSettings:
        if s.id.startswith("_"):
            # Hidden settings are plain attributes; a property of that name
            # would recurse forever on assignment (0.2.0's crash).
            check(not isinstance(getattr(type(synth), s.id, None), property), f"{s.id} is a plain attribute")
        else:
            check(isinstance(getattr(type(synth), s.id, None), property), f"{s.id} is a property NVDA made")
    # Every setting reads back, as the settings dialog and ring read them.
    for s in synth.supportedSettings:
        getattr(synth, s.id)
    check(True, "every setting reads back")
    check(len(synth.availableSamplerates) == 20, "20 sample rates in the combo box")

    # The user moves sliders; NVDA saves, then a restart loads.
    synth.flutter = 25
    synth.headSize = 120
    synth.sampleRate = "22050"
    synth.saveSettings()
    check(section["flutter"] == 25 and section["headSize"] == 120 and section["sampleRate"] == "22050",
          "changed settings are saved")
    synth.terminate()

    synth = driver.SynthDriver()
    synth.initSettings()
    check((synth.flutter, synth.headSize, synth.sampleRate) == (25, 120, "22050"), "a restart loads them")

    # Speaking: audio arrives, at the chosen rate.
    done = threading.Event()
    def on_done(synth=None):
        done.set()

    # NVDA holds handlers weakly, so this one is kept in a variable.
    synthDriverHandler.synthDoneSpeaking.register(on_done)
    synth.speak(["Hello from Klatter in NVDA."])
    finished = done.wait(10)
    check(finished, "speech finishes")
    if not finished:
        print("events:", events[-5:], "queue:", synth._queue.qsize(), "player bytes:", synth._player.bytes)
    check(synth._player.rate == 22050 and synth._player.bytes > 22050, "audio comes at 22050 Hz")
    errors = [e for e in events if e[0] == "error"]
    check(not errors, f"no errors logged {errors}")
    synth.terminate()
    print(f"{len(failures)} FAILED" if failures else "All real-NVDA checks passed.")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
