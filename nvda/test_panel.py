"""Builds the Klatter settings panel (globalPlugins/klatterSettings) with real
wxPython and the real driver, started on NVDA's own settings code as
test_real_nvda.py does: with Klatter speaking, and with another synthesizer
speaking, where the page once had every control disabled, so a screen reader
found it empty.

Usage: python3 nvda/test_panel.py path/to/klatter.dll path/to/nvda/source
(needs wxPython: uv run --with wxpython ...)"""
import sys, types, importlib.util, traceback
sys.argv = [sys.argv[0], sys.argv[1], sys.argv[2]]
from pathlib import Path
spec = importlib.util.spec_from_file_location("real", Path(__file__).with_name("test_real_nvda.py"))
real = importlib.util.module_from_spec(spec); spec.loader.exec_module(real)
config, synthDriverHandler = real.load_nvda()
dspec = importlib.util.spec_from_file_location("klatter_driver", real.DRIVER)
driver = importlib.util.module_from_spec(dspec); dspec.loader.exec_module(driver)
driver.DLL_PATH = str(real.DLL)
synth = driver.SynthDriver(); synth.initSettings(); synth.loadSettings()
synthDriverHandler._curSynth = synth
import wx
app = wx.App(False)
frame = wx.Frame(None)
class SettingsPanel(wx.Panel):
    """NVDA's SettingsPanel, reduced to how it calls makeSettings."""
    def __init__(self, parent):
        super().__init__(parent)
        sizer = wx.BoxSizer(wx.VERTICAL)
        self.makeSettings(sizer)
        self.SetSizer(sizer)
    def onPanelActivated(self):
        self.Show()
gui = types.ModuleType("gui"); sd = types.ModuleType("gui.settingsDialogs")
sd.SettingsPanel = SettingsPanel; sd.NVDASettingsDialog = types.SimpleNamespace(categoryClasses=[])
sys.modules["gui"] = gui; sys.modules["gui.settingsDialogs"] = sd
sys.modules["globalPluginHandler"] = types.SimpleNamespace(GlobalPlugin=object)
pspec = importlib.util.spec_from_file_location("panel", str(real.ROOT / "nvda/addon/globalPlugins/klatterSettings/__init__.py"))
panel_mod = importlib.util.module_from_spec(pspec)
failures = []
def check(ok, msg):
    print(("ok: " if ok else "FAIL: ") + msg)
    if not ok: failures.append(msg)
try:
    pspec.loader.exec_module(panel_mod)
    for active in (True, False):
        synthDriverHandler._curSynth = synth if active else None
        if not active:
            config.conf["speech"].pop("klatter", None)  # never chosen: no saved section
        p = panel_mod.KlatterSettingsPanel(frame)
        p.onPanelActivated()
        controls = [c for c in p.GetChildren() if isinstance(c, (wx.CheckBox, wx.SpinCtrl))]
        who = "with Klatter speaking" if active else "with another synthesizer speaking"
        check(len(controls) == 4 and p.pitch_hz_mode.IsEnabled() and p.wpm_mode.IsEnabled(), f"both check boxes enabled {who}")
        check(p.panelDescription.startswith("Klatter is the" if active else "Klatter isn't"), f"the page says so {who}")
        p.wpm_mode.SetValue(True); p.wpm_value.SetValue(450)
        p.on_wpm_changed(wx.CommandEvent())
        p.pitch_hz_mode.SetValue(True); p.pitch_hz_value.SetValue(150)
        p.on_pitch_changed(wx.CommandEvent())
        p.onSave()
        if active:
            check((synth._rateInWpm, synth._rateWpm, synth._pitchInHertz, synth._pitchHertz) == (True, 450, True, 150), "the running Klatter takes them")
            check(config.conf["speech"]["klatter"]["_rateWpm"] == 450, "and NVDA's configuration has them")
        else:
            sec = config.conf["speech"]["klatter"]
            check((sec["_rateInWpm"], sec["_rateWpm"], sec["_pitchInHertz"], sec["_pitchHertz"]) == (True, 450, True, 150), "they are saved for when Klatter is chosen")
            p2 = panel_mod.KlatterSettingsPanel(frame)
            check((p2.wpm_value.GetValue(), p2.pitch_hz_value.GetValue(), p2.wpm_mode.GetValue()) == (450, 150, True), "and the page shows them next time")
            # Choosing Klatter afterwards loads them.
            synth2 = driver.SynthDriver(); synth2.initSettings()
            check((synth2._rateInWpm, synth2._rateWpm, synth2._pitchHertz) == (True, 450, 150), "Klatter speaks with them once chosen")
            synth2.terminate()
except Exception:
    traceback.print_exc(); failures.append("exception")
synth.terminate()
print(f"{len(failures)} FAILED" if failures else "All panel checks passed.")
sys.exit(1 if failures else 0)
