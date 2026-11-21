// Klatter on the web: rate in words per minute, pitch in hertz, the 20 sample
// rates and every voice-quality setting, speaking with Klatter compiled to
// WebAssembly (klatter.wasm, from plat/web).
"use strict";

const SAMPLE_RATES = [8000, 10000, 11000, 11025, 12000, 16000, 20000, 22000, 22050, 24000,
  32000, 40000, 44000, 44100, 48000, 64000, 80000, 88000, 88200, 96000];
const DEFAULT_RATE = 16000;
const SAMPLE_TEXT =
  "Hello! I'm Klatter, a formant speech synthesizer in the tradition of Klatt and Eloquence, " +
  "speaking in your browser.\n\nTry a faster rate, a different pitch, or the voice quality " +
  "settings below: a larger head size, some breathiness, or a little flutter.\n";

const $ = (id) => document.getElementById(id);
const text = $("text"), sampleRate = $("sample-rate"), status = $("status");
const numbers = { wpm: $("wpm"), pitch: $("pitch"), inflection: $("inflection"), volume: $("volume") };
const playButton = $("play"), pauseButton = $("pause"), stopButton = $("stop"), wavButton = $("wav");

let module = null, worker = null, parameters = [];
let context = null, sources = [], nextTime = 0;
let speaking = false, playing = false, paused = false;
// The buttons stay enabled while speaking, so the keyboard focus never falls off
// one: each does nothing when there is nothing for it to do.

function store(key, value) {
  try { localStorage.setItem("klatter." + key, value); } catch (e) { /* private browsing */ }
}
function stored(key) {
  try { return localStorage.getItem("klatter." + key); } catch (e) { return null; }
}
function say(message) { status.textContent = message; }

// A typed number past either end goes to the end, and the page says so.
function clampInput(input, name) {
  const min = Number(input.min), max = Number(input.max);
  let v = Number(input.value);
  if (!Number.isFinite(v) || input.value === "") v = Number(input.defaultValue);
  const c = Math.min(max, Math.max(min, v));
  if (c !== v) say(name + " " + c + ", the " + (v < c ? "lowest" : "highest") + " allowed.");
  input.value = String(c);
  return c;
}

function buildQuality() {
  const box = $("quality");
  box.textContent = "";
  for (const p of parameters) {
    const id = "q-" + p.code;
    const div = document.createElement("div");
    const label = document.createElement("label");
    label.htmlFor = id;
    label.textContent = p.name;
    const range = document.createElement("span");
    range.className = "value";
    range.textContent = " (" + p.min + " to " + p.max + (p.unit ? " " + p.unit : "") + ")";
    label.appendChild(range);
    const input = document.createElement("input");
    Object.assign(input, { id, type: "number", min: p.min, max: p.max, step: p.step });
    input.defaultValue = String(p.default);
    const saved = stored("q." + p.code);
    input.value = saved !== null ? saved : String(p.default);
    const help = document.createElement("p");
    help.className = "help";
    help.id = id + "-help";
    help.textContent = p.help;
    input.setAttribute("aria-describedby", help.id);
    input.addEventListener("change", () => store("q." + p.code, String(clampInput(input, p.name))));
    div.append(label, input, help);
    box.appendChild(div);
  }
}

function quality() {
  return parameters.map((p) => {
    const v = Number($("q-" + p.code).value);
    return Number.isFinite(v) ? v : p.default;
  });
}

function startWorker() {
  worker = new Worker("worker.js");
  worker.onmessage = onWorkerMessage;
  worker.onerror = (event) => say("Klatter stopped with an error: " + (event.message || "unknown"));
  worker.postMessage({ type: "load", module });
}

function onWorkerMessage(event) {
  const message = event.data;
  if (message.type === "ready") {
    if (parameters.length === 0) {
      parameters = message.parameters;
      buildQuality();
      for (const b of [playButton, pauseButton, stopButton, wavButton]) b.disabled = false;
      say("Ready. Press Play or F5.");
    }
  } else if (message.type === "audio") {
    schedule(message.samples);
  } else if (message.type === "done") {
    speaking = false;
    if (sources.length === 0) finished();
  } else if (message.type === "wav") {
    speaking = false;
    const rate = Number(sampleRate.value);
    download(wavFile(message.samples, rate), "Klatter " + rate + " Hz.wav");
    say("Exported " + (message.samples.length / rate).toFixed(1) + " seconds as WAV.");
  }
}

function schedule(samples) {
  if (!playing) return;
  const buffer = context.createBuffer(1, samples.length, Number(sampleRate.value));
  buffer.copyToChannel(samples, 0);
  const source = context.createBufferSource();
  source.buffer = buffer;
  source.connect(context.destination);
  nextTime = Math.max(nextTime, context.currentTime + 0.03);
  source.start(nextTime);
  nextTime += buffer.duration;
  sources.push(source);
  source.onended = () => {
    sources = sources.filter((s) => s !== source);
    if (sources.length === 0 && !speaking && playing) finished();
  };
}

function finished() {
  playing = false;
  paused = false;
  pauseButton.textContent = "Pause";
  say("Done.");
}

function request(mode) {
  const start = text.selectionStart, end = text.selectionEnd;
  const part = mode === "play" && end > start ? text.value.slice(start, end) : text.value;
  return {
    type: "speak",
    mode,
    text: part,
    wpm: clampInput(numbers.wpm, "Rate"),
    pitch: clampInput(numbers.pitch, "Pitch"),
    inflection: clampInput(numbers.inflection, "Inflection") / 100,
    volume: clampInput(numbers.volume, "Volume") / 100,
    sampleRate: Number(sampleRate.value),
    quality: quality(),
  };
}

async function play() {
  if (!module || parameters.length === 0) return;
  if (paused) return pause();
  stop(true);
  if (!context) context = new AudioContext();
  if (context.state === "suspended") await context.resume();
  playing = true;
  speaking = true;
  nextTime = 0;
  say("Playing.");
  worker.postMessage(request("play"));
}

function pause() {
  if (!playing || !context) return;
  if (paused) {
    context.resume();
    paused = false;
    pauseButton.textContent = "Pause";
    say("Resumed.");
  } else {
    context.suspend();
    paused = true;
    pauseButton.textContent = "Resume";
    say("Paused. Press Resume, F6 or F5 to go on.");
  }
}

function stop(quiet) {
  const was = playing || speaking;
  for (const s of sources) {
    s.onended = null;
    try { s.stop(); } catch (e) { /* not started */ }
  }
  sources = [];
  if (speaking) {
    // The engine runs to the end of what it was given, so a new worker takes over.
    worker.terminate();
    startWorker();
  }
  speaking = false;
  playing = false;
  if (paused && context) context.resume();
  paused = false;
  pauseButton.textContent = "Pause";
  if (was && !quiet) say("Stopped.");
}

function exportWav() {
  if (!module || parameters.length === 0 || (speaking && !playing)) return;
  stop(true);
  speaking = true;
  say("Making the WAV file…");
  worker.postMessage(request("wav"));
}

// 16-bit mono PCM.
function wavFile(samples, rateHz) {
  const bytes = new ArrayBuffer(44 + samples.length * 2);
  const view = new DataView(bytes);
  const ascii = (at, s) => { for (let i = 0; i < s.length; i++) view.setUint8(at + i, s.charCodeAt(i)); };
  ascii(0, "RIFF"); view.setUint32(4, 36 + samples.length * 2, true); ascii(8, "WAVE");
  ascii(12, "fmt "); view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true);
  view.setUint32(24, rateHz, true); view.setUint32(28, rateHz * 2, true); view.setUint16(32, 2, true); view.setUint16(34, 16, true);
  ascii(36, "data"); view.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    view.setInt16(44 + i * 2, Math.round(Math.max(-1, Math.min(1, samples[i])) * 32767), true);
  }
  return new Blob([bytes], { type: "audio/wav" });
}

function download(blob, name) {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = name;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(link.href), 10000);
}

// ---- Setting up ---------------------------------------------------------------

for (const r of SAMPLE_RATES) sampleRate.add(new Option(r + " hertz", String(r)));
sampleRate.value = String(SAMPLE_RATES.includes(Number(stored("sampleRate"))) ? Number(stored("sampleRate")) : DEFAULT_RATE);
sampleRate.addEventListener("change", () => store("sampleRate", sampleRate.value));
for (const [key, input] of Object.entries(numbers)) {
  const v = stored(key);
  if (v !== null) input.value = v;
  input.addEventListener("change", () => store(key, String(clampInput(input, input.labels[0].firstChild.textContent.trim()))));
}
text.value = stored("text") ?? SAMPLE_TEXT;
let saveTimer = 0;
text.addEventListener("input", () => {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => store("text", text.value), 500);
});

playButton.addEventListener("click", play);
pauseButton.addEventListener("click", pause);
stopButton.addEventListener("click", () => stop(false));
wavButton.addEventListener("click", exportWav);
$("reset").addEventListener("click", () => {
  for (const p of parameters) {
    $("q-" + p.code).value = String(p.default);
    store("q." + p.code, String(p.default));
  }
  say("Voice quality is back to Klatter's own.");
});
$("open").addEventListener("click", () => $("file").click());
$("file").addEventListener("change", async () => {
  const file = $("file").files[0];
  if (!file) return;
  text.value = await file.text();
  store("text", text.value);
  $("file").value = "";
  say("Opened " + file.name + ".");
  text.focus();
});
$("save").addEventListener("click", () => {
  download(new Blob([text.value], { type: "text/plain;charset=utf-8" }), "Klatter.txt");
  say("Saved Klatter.txt to your downloads.");
});
document.addEventListener("keydown", (event) => {
  if (event.altKey || event.ctrlKey || event.metaKey) return;
  if (event.key === "F5") { event.preventDefault(); play(); }
  else if (event.key === "F6") { event.preventDefault(); pause(); }
  else if (event.key === "F7") { event.preventDefault(); stop(false); }
});

(async () => {
  try {
    module = await WebAssembly.compileStreaming(fetch("klatter.wasm"));
  } catch (e) {
    try {
      module = await WebAssembly.compile(await (await fetch("klatter.wasm")).arrayBuffer());
    } catch (e2) {
      say("Could not load Klatter: " + e2.message);
      return;
    }
  }
  startWorker();
})();
