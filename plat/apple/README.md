# Klatter for VoiceOver on the Mac, iPhone and iPad

Klatter as a system voice: VoiceOver (on the Mac, VoiceOver Utility, Speech, Add Voice; on iPhone and iPad,
Settings, Accessibility, VoiceOver, Speech, Voice), Spoken Content and every app that speaks. One app and one
speech synthesis extension, from the same sources for both.

The app's settings, as the NVDA add-on's: rate in words per minute (30–1920, 219 natural) and whether
VoiceOver's rate scales it; an exact base pitch in hertz (40–400); inflection; flutter (a slider, off by
default); volume; the 20 sample rates; and every voice-quality setting, each typed exactly or stepped, with
Reset. Its test box speaks directly or through the system, and Save as WAV writes a 16-bit WAV at the chosen
rate (on iPhone and iPad, into Klatter's folder in the Files app). macOS 13 or newer (Apple silicon and Intel),
iOS and iPadOS 16 or newer.

```
./build.sh mac install    # the Mac
./build.sh device         # a real iPhone or iPad (install with Xcode's Devices window)
./build.sh simulator      # the iOS Simulator
```

Needs Rust (`rustup target add aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim
x86_64-apple-ios`), Xcode and `xcodegen`. It links `klatter-ffi` as a static library. Signing uses the team in
`project.yml`; to build for yourself, put your own team there and your own bundle identifier and App Groups
(`<team>.<bundle id>` on the Mac, `group.<bundle id>` on iOS). The system lists a third-party voice only when it
is signed with a real Apple certificate; with a free account an iPhone app lasts 7 days.
