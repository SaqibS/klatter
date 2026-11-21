// Speaks in the background for Klatter on the web. The page sends the compiled
// engine once, then one request at a time; audio goes back in pieces as it is
// made. To stop, the page ends this worker and starts another.
"use strict";

let engine = null;
let pending = [];
let pendingLength = 0;
let mode = "play";
let collected = [];
// Pieces of about a tenth of a second, so the page schedules few of them.
let minimum = 2205;

function flush() {
  if (pendingLength === 0) return;
  const samples = new Float32Array(pendingLength);
  let at = 0;
  for (const piece of pending) {
    samples.set(piece, at);
    at += piece.length;
  }
  pending = [];
  pendingLength = 0;
  if (mode === "play") {
    postMessage({ type: "audio", samples }, [samples.buffer]);
  } else {
    collected.push(samples);
  }
}

self.onmessage = async (event) => {
  const message = event.data;
  if (message.type === "load") {
    const instance = await WebAssembly.instantiate(message.module, {
      env: {
        klatter_audio(pointer, count) {
          const ints = new Int16Array(engine.memory.buffer, pointer, count);
          const piece = new Float32Array(count);
          for (let i = 0; i < count; i++) piece[i] = ints[i] / 32768;
          pending.push(piece);
          pendingLength += piece.length;
          if (pendingLength >= minimum) flush();
          return 1;
        },
      },
    });
    engine = instance.exports;
    engine.klatter_parameters();
    postMessage({ type: "ready", parameters: JSON.parse(readMessage()) });
    return;
  }
  if (message.type === "speak") {
    mode = message.mode;
    collected = [];
    minimum = Math.round(message.sampleRate / 10);
    const bytes = new TextEncoder().encode(message.text);
    const pointer = engine.klatter_alloc(bytes.length);
    new Uint8Array(engine.memory.buffer, pointer, bytes.length).set(bytes);
    const q = message.quality;
    const qPointer = engine.klatter_alloc(q.length * 8);
    new Float64Array(engine.memory.buffer, qPointer, q.length).set(q);
    engine.klatter_say(pointer, bytes.length, message.wpm, message.pitch, message.inflection,
      message.volume, message.sampleRate, qPointer, q.length);
    engine.klatter_free(pointer, bytes.length);
    engine.klatter_free(qPointer, q.length * 8);
    flush();
    const error = "";
    if (mode === "wav") {
      let total = 0;
      for (const piece of collected) total += piece.length;
      const samples = new Float32Array(total);
      let at = 0;
      for (const piece of collected) {
        samples.set(piece, at);
        at += piece.length;
      }
      collected = [];
      postMessage({ type: "wav", samples, error }, [samples.buffer]);
    } else {
      postMessage({ type: "done", error });
    }
  }
};

function readMessage() {
  const pointer = engine.klatter_message();
  const length = engine.klatter_message_length();
  return new TextDecoder().decode(new Uint8Array(engine.memory.buffer, pointer, length));
}
