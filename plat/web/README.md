# Klatter on the web

Klatter in a browser: rate in words per minute (30–1920), pitch in hertz, inflection, volume, the 20 sample
rates, and every voice-quality setting with its range and help (flutter off by default). Play (F5), Pause (F6)
and Stop (F7), Export WAV, and opening and saving text. It runs entirely in the browser: the engine is compiled
to WebAssembly and speaks in a Web Worker, so nothing is sent anywhere. Text and settings are remembered.

```
rustup target add wasm32-unknown-unknown
./build-web.sh
python3 -m http.server -d www 8000     # then open http://localhost:8000
```

Any static web server works, GitHub Pages included; a page opened from the disk (`file://`) can't load the
engine.
