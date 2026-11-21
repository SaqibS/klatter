"""Klatter settings, as a category of NVDA's settings dialog (adapted from
Klattara's, which follows Reloq-C's Eloquence panel).

The precise pitch and rate live here, not in Speech settings, as Reloq-C
does with its Eloquence C panel. The settings are hidden from NVDA's
automatic Speech panel (their ids start with an underscore, which the
dialog skips), and this panel gives them exact numeric inputs instead of
0 to 100 sliders, which cannot land on an exact hertz or words-per-minute
value.
"""

import addonHandler
import globalPluginHandler
from gui.settingsDialogs import NVDASettingsDialog, SettingsPanel
import synthDriverHandler
import wx

addonHandler.initTranslation()

# The exact rate's range in words per minute (Klatter's normal is 219), and
# the base pitch's in hertz (normal 100), as the driver keeps them.
MIN_WPM, MAX_WPM = 30, 1920
MIN_HZ, MAX_HZ = 40, 400


def _whole(value, low, high):
	"""A value for a spin control: a whole number within its range. wx.SpinCtrl
	refuses a float with TypeError, even a whole one such as 210.0, and a
	voice's pitch used to reach the synth as one; that raised part way through
	building or refreshing the panel, which left the rate controls showing old,
	unrelated values."""
	return int(min(high, max(low, round(value))))


class KlatterSettingsPanel(SettingsPanel):
	title = _("Klatter")

	def makeSettings(self, sizer):
		synth = synthDriverHandler.getSynth()
		self.synth = synth if synth is not None and synth.name == "klatter" else None
		self.pitch_dirty = False
		self.wpm_dirty = False
		self._saved_pitch = (
			(bool(self.synth._pitchInHertz), _whole(self.synth._pitchHertz, MIN_HZ, MAX_HZ))
			if self.synth
			else None
		)
		self._saved_wpm = (
			(bool(self.synth._rateInWpm), _whole(self.synth._rateWpm, MIN_WPM, MAX_WPM))
			if self.synth
			else None
		)
		self._updating = False
		sizer.Add(wx.StaticText(self, label=_("Exact base pitch")), 0, wx.ALL, 10)
		self.pitch_hz_mode = wx.CheckBox(self, label=_("Use base pitch in &hertz"))
		self.pitch_hz_mode.SetValue(self.synth._pitchInHertz if self.synth else False)
		sizer.Add(self.pitch_hz_mode, 0, wx.ALL, 10)
		pitch_label = wx.StaticText(self, label=_("Base pitch in hertz (40–400):"))
		sizer.Add(pitch_label, 0, wx.LEFT | wx.RIGHT, 10)
		self.pitch_hz_value = wx.SpinCtrl(
			self,
			min=MIN_HZ,
			max=MAX_HZ,
			initial=_whole(self.synth._pitchHertz, MIN_HZ, MAX_HZ) if self.synth else 100,
		)
		self.pitch_hz_value.SetName(pitch_label.GetLabelText())
		sizer.Add(self.pitch_hz_value, 0, wx.ALL, 10)
		sizer.Add(
			wx.StaticText(
				self,
				label=_(
					"Changing NVDA's normal pitch slider switches back to the normal pitch scale."
				),
			),
			0,
			wx.EXPAND | wx.LEFT | wx.RIGHT | wx.BOTTOM,
			10,
		)
		self.pitch_hz_mode.Bind(wx.EVT_CHECKBOX, self.on_pitch_changed)
		self.pitch_hz_value.Bind(wx.EVT_SPINCTRL, self.on_pitch_changed)
		self.pitch_hz_value.Bind(wx.EVT_TEXT, self.on_pitch_changed)
		sizer.Add(wx.StaticText(self, label=_("Exact speech rate")), 0, wx.ALL, 10)
		self.wpm_mode = wx.CheckBox(self, label=_("Use speech rate in &words per minute"))
		self.wpm_mode.SetValue(self.synth._rateInWpm if self.synth else False)
		sizer.Add(self.wpm_mode, 0, wx.ALL, 10)
		wpm_label = wx.StaticText(
			self, label=_("Speech rate in words per minute (30–1920):")
		)
		sizer.Add(wpm_label, 0, wx.LEFT | wx.RIGHT, 10)
		self.wpm_value = wx.SpinCtrl(
			self,
			min=MIN_WPM,
			max=MAX_WPM,
			initial=_whole(self.synth._rateWpm, MIN_WPM, MAX_WPM) if self.synth else 219,
		)
		self.wpm_value.SetName(wpm_label.GetLabelText())
		sizer.Add(self.wpm_value, 0, wx.ALL, 10)
		sizer.Add(
			wx.StaticText(
				self,
				label=_(
					"Changing NVDA's normal rate slider switches back to the normal rate scale."
				),
			),
			0,
			wx.EXPAND | wx.LEFT | wx.RIGHT | wx.BOTTOM,
			10,
		)
		self.wpm_mode.Bind(wx.EVT_CHECKBOX, self.on_wpm_changed)
		self.wpm_value.Bind(wx.EVT_SPINCTRL, self.on_wpm_changed)
		self.wpm_value.Bind(wx.EVT_TEXT, self.on_wpm_changed)
		if not self.synth:
			sizer.Add(
				wx.StaticText(
					self, label=_("Select Klatter as the synthesizer first.")
				),
				0,
				wx.EXPAND | wx.LEFT | wx.RIGHT | wx.BOTTOM,
				10,
			)
		self.enable_controls(bool(self.synth))

	def enable_controls(self, enabled):
		self.pitch_hz_mode.Enable(enabled)
		self.pitch_hz_value.Enable(enabled and self.pitch_hz_mode.GetValue())
		self.wpm_mode.Enable(enabled)
		self.wpm_value.Enable(enabled and self.wpm_mode.GetValue())

	def current_synth(self):
		return self.synth is not None and synthDriverHandler.getSynth() is self.synth

	def on_pitch_changed(self, event):
		synth = self.synth if self.current_synth() else None
		self.pitch_hz_value.Enable(bool(synth) and self.pitch_hz_mode.GetValue())
		if not self._updating and synth:
			value = self.pitch_hz_value.GetValue()
			if isinstance(value, int) and MIN_HZ <= value <= MAX_HZ:
				synth.configure_pitch_hertz(bool(self.pitch_hz_mode.GetValue()), value)
				self.pitch_dirty = True
		event.Skip()

	def on_wpm_changed(self, event):
		synth = self.synth if self.current_synth() else None
		self.wpm_value.Enable(bool(synth) and self.wpm_mode.GetValue())
		if not self._updating and synth:
			value = self.wpm_value.GetValue()
			if isinstance(value, int) and MIN_WPM <= value <= MAX_WPM:
				synth.configure_rate_wpm(bool(self.wpm_mode.GetValue()), value)
				self.wpm_dirty = True
		event.Skip()

	def onPanelActivated(self):
		# Refresh from the synth: the user may have changed the voice or the
		# normal sliders since the panel was built, and moving a normal slider
		# switches a precise mode off. The super call shows the panel: NVDA
		# hides it when the category is left, and without it the panel stays
		# hidden and the category looks like an empty scroll panel.
		super().onPanelActivated()
		if self.current_synth():
			self._updating = True
			try:
				self.pitch_hz_mode.SetValue(bool(self.synth._pitchInHertz))
				self.pitch_hz_value.SetValue(_whole(self.synth._pitchHertz, MIN_HZ, MAX_HZ))
				self.wpm_mode.SetValue(bool(self.synth._rateInWpm))
				self.wpm_value.SetValue(_whole(self.synth._rateWpm, MIN_WPM, MAX_WPM))
			finally:
				self._updating = False
			self.enable_controls(True)

	def onSave(self):
		if self.current_synth():
			if self.pitch_dirty:
				# Clamp, never refuse: a value typed past the ends (20 or 2000
				# words per minute, say) must not raise and leave OK dead.
				# The spin control itself holds the typed value until it loses
				# focus, so OK can arrive while it is still out of range.
				value = max(MIN_HZ, min(MAX_HZ, self.pitch_hz_value.GetValue()))
				self.synth.configure_pitch_hertz(
					self.pitch_hz_mode.GetValue(), value
				)
			if self.wpm_dirty:
				value = max(MIN_WPM, min(MAX_WPM, self.wpm_value.GetValue()))
				self.synth.configure_rate_wpm(
					self.wpm_mode.GetValue(), value
				)
			self.synth.saveSettings()
			self._saved_pitch = (self.synth._pitchInHertz, self.synth._pitchHertz)
			self._saved_wpm = (self.synth._rateInWpm, self.synth._rateWpm)
			self.pitch_dirty = self.wpm_dirty = False

	def _restore_preview(self):
		synth = self.synth if self.current_synth() else None
		if synth:
			if self.pitch_dirty and self._saved_pitch is not None:
				synth.configure_pitch_hertz(*self._saved_pitch)
			if self.wpm_dirty and self._saved_wpm is not None:
				synth.configure_rate_wpm(*self._saved_wpm)
		self.pitch_dirty = self.wpm_dirty = False

	def onDiscard(self):
		self._restore_preview()


class GlobalPlugin(globalPluginHandler.GlobalPlugin):
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)
		if KlatterSettingsPanel not in NVDASettingsDialog.categoryClasses:
			NVDASettingsDialog.categoryClasses.append(KlatterSettingsPanel)

	def terminate(self):
		if KlatterSettingsPanel in NVDASettingsDialog.categoryClasses:
			NVDASettingsDialog.categoryClasses.remove(KlatterSettingsPanel)
		super().terminate()
