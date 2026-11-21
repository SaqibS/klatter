# Klatter for Android

Klatter as an Android text-to-speech engine, for TalkBack, other screen readers, braille notetakers and every
app that speaks, from Android 4.0 (API 14) on phones, tablets, notetakers and Wear OS watches. Install the APK,
choose Klatter as the preferred engine (Settings, Accessibility, Text-to-speech output), and it speaks:
everything is in the app, nothing is downloaded.

Settings, as in the NVDA add-on and on the Mac: rate from 30 to 1920 words per minute (219 natural), following
the screen reader's rate or exact; an exact base pitch in hertz (40 to 400); inflection; a flutter slider (off by
default); volume; the 20 sampling rates (48000 at most before Android 5, the most its audio can play); every
voice-quality setting (head size, roughness, breathiness, open quotient, spectral tilt, the source levels,
accent size, baseline fall, F4 to F6 and their bandwidths); pronunciation dictionaries in ARPAbet ("klatter K L
AE1 T ER0"); and a test box that speaks directly or through Android, and saves WAV files. Choices such as the
sampling rate are buttons that open a list, which notetakers' screen readers reach more easily than drop-downs.

## The builds

| Flavor | Android | Processors | For |
|---|---|---|---|
| `anyDevice` | 4.0+ | all four | any device: install this if unsure |
| `legacy` | 4.0+ | 32-bit ARM, x86 | Android 4.x phones and notetakers (smaller) |
| `modernArm64` | 5.0+ | 64-bit ARM | most phones and notetakers today |
| `modernArm32` | 5.0+ | 32-bit ARM | older or lower-cost devices |
| `modernX86_64` | 5.0+ | 64-bit Intel | emulators, Chromebooks |
| `modernX86` | 5.0+ | 32-bit Intel | older Intel tablets, emulators |
| `watch` | 7.1+ (Wear OS) | 64-bit and 32-bit ARM | Pixel Watch, Galaxy Watch and others |

All share one application ID and version code, so any one replaces another.

## Building

Needs the Android SDK (platform 35) and NDK (27 or newer), Java 17, Python 3 and Rust with the Android targets:

```
rustup target add armv7-linux-androideabi aarch64-linux-android i686-linux-android x86_64-linux-android
./build-native.sh          # the engine, into app/src/main/jniLibs/<abi>/libklatter.so
./gradlew assembleRelease  # every flavor, into app/build/outputs/apk/<flavor>/release/
```

Set `ANDROID_HOME` (or `sdk.dir=...` in `local.properties`). The release builds are signed with the debug key so
they install straight away; use your own key to publish.

## How it fits together

- `rust/` is a static library: the engine's C interface (`ffi/klatter.h`, from `../../ffi`) and an allocator that
  Android 4.0 has the functions for (Rust's own asks for `posix_memalign`, which is Android 4.2).
- `jni/klatter_jni.c` is the bridge to Java. Text crosses as UTF-8 bytes; speaking pulls the engine's audio a
  block at a time and hands each block to Java, which can stop it at once. On 32-bit ARM it also supplies
  `dl_iterate_phdr`, which Android only has there from 5.0. `build-native.sh` checks every system function each
  library imports against bionic's records (`tools/`) and fails if any is newer than the oldest Android it is for.
- `app/` is plain Java on Android's own APIs (no support libraries): `KlatterTtsService` is the engine,
  `Synthesizer` speaks for it, the settings screen and WAV export alike, and `SettingsActivity` is the settings
  screen, built from standard controls for TalkBack and notetakers. `Parameters` reads the voice-quality
  settings from the engine itself, so a setting added to `src/quality.rs` appears here too.

## Testing

`adb shell am start -a com.dgomeze42.klatter.SELF_TEST -n com.dgomeze42.klatter/.SettingsActivity` runs a check
through Android's speech service (Android 5.0 and newer): normal and double speed, flutter off and on, every
sampling rate from 48000 up and 8000, the time to the first audio, and the dictionary. The results go to the log
under `KlatterSelfTest`.
