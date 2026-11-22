// The Klatter app, for macOS, iPhone and iPad: carries the speech synthesis
// extension (which is what puts Klatter's voice in VoiceOver and Spoken
// Content), and edits the settings the extension speaks with.

import AVFoundation
import SwiftUI
import UniformTypeIdentifiers

@main
struct KlatterApp: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup("Klatter") {
            #if os(macOS)
            ContentView(model: model)
                .frame(minWidth: 520, minHeight: 560)
            #else
            NavigationStack {
                ContentView(model: model)
                    .navigationTitle("Klatter")
            }
            #endif
        }
        #if os(macOS)
        .windowResizability(.contentSize)
        #endif
    }
}

@MainActor
final class AppModel: ObservableObject {

    @Published var settings: KlatterSettings {
        didSet {
            if settings != oldValue { persist() }
        }
    }
    @Published var testText = "Hello. This is Klatter, a formant speech synthesizer in the tradition of Klatt and Eloquence, speaking on \(platformName)."
    @Published var status = ""

    private let engine = AVAudioEngine()
    private var speaker: KlatterSpeaker?
    private var source: AVAudioSourceNode?
    private let synthesizer = AVSpeechSynthesizer()

    init() {
        settings = KlatterSettingsStore.shared.current
        // Write the file once even if nothing changes, so the extension finds
        // it; and ask the system to read the voice list again, which is what
        // makes the voices appear after an install or an update.
        persist()
        AVSpeechSynthesisProviderVoice.updateSpeechVoices()
    }

    private func persist() {
        do {
            try KlatterSettingsStore.shared.save(settings)
        } catch {
            status = "Could not save the settings: \(error.localizedDescription)"
        }
    }

    var systemVoice: AVSpeechSynthesisVoice? {
        // The system's identifier has the extension's in front of ours.
        let own = "com.dgomeze42.klatter.voice.klatter"
        return AVSpeechSynthesisVoice.speechVoices().first { $0.identifier == own || $0.identifier.hasSuffix("." + own) }
    }

    /// Speaks the test text straight through the engine, with these settings.
    func speakDirectly() {
        stop()
        #if os(iOS)
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        try? AVAudioSession.sharedInstance().setActive(true)
        #endif
        // At the chosen rate itself; the audio engine converts for the device.
        let rate = Double(settings.sampleRate)
        let speaker = KlatterSpeaker(outputRate: rate)
        let format = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1)!
        let node = AVAudioSourceNode(format: format) { _, _, frameCount, buffers in
            let list = UnsafeMutableAudioBufferListPointer(buffers)
            if let out = list[0].mData?.assumingMemoryBound(to: Float.self) {
                _ = speaker.render(into: out, frames: Int(frameCount))
            }
            return noErr
        }
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: format)
        do {
            try engine.start()
        } catch {
            status = "Could not start audio: \(error.localizedDescription)"
            return
        }
        self.speaker = speaker
        source = node
        speaker.start(items: KlatterSynthesis.items(forPlainText: testText), settings: settings)
        status = "Speaking directly at \(settings.sampleRate) hertz."
    }

    @Published var isSaving = false

    /// Writes the text box's text as a WAV file, with the chosen rate,
    /// pitch and sample rate: on a Mac where you choose; on iPhone and iPad into
    /// Klatter's folder in the Files app.
    func saveWAV() {
        let name = "Klatter \(settings.sampleRate) Hz.wav"
        #if os(macOS)
        let panel = NSSavePanel()
        panel.title = "Save Speech as WAV"
        panel.allowedContentTypes = [.wav]
        panel.nameFieldStringValue = name
        guard panel.runModal() == .OK, let url = panel.url else { return }
        #else
        guard let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
        let url = documents.appendingPathComponent(name)
        #endif
        let text = testText, settings = settings
        isSaving = true
        status = "Saving \(url.lastPathComponent)…"
        Task.detached(priority: .userInitiated) {
            let result: String
            do {
                let seconds = try KlatterSynthesis.writeWAV(
                    items: KlatterSynthesis.items(forPlainText: text), settings: settings, to: url)
                result = String(format: "Saved %@: %.1f seconds, %d hertz, 16-bit mono.%@",
                                url.lastPathComponent, seconds, settings.sampleRate, savedWhere)
            } catch {
                result = "Could not save \(url.lastPathComponent): \(error.localizedDescription)"
            }
            await MainActor.run {
                self.isSaving = false
                self.status = result
            }
        }
    }

    /// Speaks the test text through the system, the way VoiceOver does.
    func speakThroughSystem() {
        stop()
        guard let systemVoice else {
            status = "Klatter is not in the system's voice list yet. Try Refresh System Voices, then wait a moment."
            return
        }
        let utterance = AVSpeechUtterance(string: testText)
        utterance.voice = systemVoice
        synthesizer.speak(utterance)
        status = "Speaking through \(platformName) with \(systemVoice.name)."
    }

    func stop() {
        synthesizer.stopSpeaking(at: .immediate)
        speaker?.cancel()
        speaker = nil
        if let source {
            engine.stop()
            engine.detach(source)
            self.source = nil
        }
    }

    func refreshSystemVoices() {
        AVSpeechSynthesisProviderVoice.updateSpeechVoices()
        let count = AVSpeechSynthesisVoice.speechVoices().filter { $0.identifier.contains("com.dgomeze42.klatter.voice.") }.count
        status = count > 0
            ? "Klatter is in the system's voice list."
            : "Asked macOS to read the voice list again. It can take a little while."
    }
}

struct ContentView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        Form {
            Section("Voice") {
                Text("Choose Klatter in \(voiceSettingsPath). These settings apply wherever Klatter speaks: VoiceOver, Spoken Content, other apps, and the test below.")
                    .font(.caption).foregroundStyle(.secondary)
            }

            Section("Rate") {
                NumberField(title: "Rate in words per minute", value: $model.settings.rateWPM,
                            range: KlatterSettings.wpmRange, step: 10, unit: "words per minute")
                Toggle("Follow VoiceOver's rate", isOn: $model.settings.followSystemRate)
                Text(model.settings.followSystemRate
                     ? "VoiceOver's rate and an app's rate scale this rate. 219 is Klatter's natural rate."
                     : "Always exactly this rate, whatever VoiceOver's rate is set to.")
                    .font(.caption).foregroundStyle(.secondary)
            }

            Section("Pitch") {
                Toggle("Use base pitch in hertz", isOn: $model.settings.useExactPitch)
                NumberField(title: "Base pitch in hertz", value: $model.settings.pitchHz,
                            range: KlatterSettings.pitchRange, step: 5, unit: "hertz")
                    .disabled(!model.settings.useExactPitch)
                Toggle("Follow VoiceOver's pitch", isOn: $model.settings.followSystemPitch)
                NumberField(title: "Inflection, percent of natural", value: $model.settings.inflection,
                            range: KlatterSettings.inflectionRange, step: 10, unit: "percent")
                if let flutter = KlatterParameter.named("fl") {
                    LabeledContent {
                        HStack {
                            Slider(value: Binding(get: { model.settings.value(of: flutter) },
                                                  set: { model.settings.set(flutter, to: $0.rounded()) }),
                                   in: flutter.range, step: flutter.step)
                                .accessibilityLabel("Flutter")
                                .accessibilityValue(model.settings.value(of: flutter) == 0 ? "off" : "\(Int(model.settings.value(of: flutter))) percent")
                            Text(model.settings.value(of: flutter) == 0 ? "Off" : "\(Int(model.settings.value(of: flutter)))%")
                                .monospacedDigit()
                                .frame(width: 44, alignment: .trailing)
                                .accessibilityHidden(true)
                        }
                    } label: {
                        Text("Flutter")
                    }
                    Text("How much the pitch wavers, slowly and unevenly. Off unless you turn it up; 25 is what Klatt and Klatt found natural.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }

            Section("Audio") {
                Picker("Sample rate", selection: $model.settings.sampleRate) {
                    ForEach(KlatterSettings.sampleRates, id: \.self) { rate in
                        Text(rate == KlatterSettings.defaultSampleRate ? "\(rate) hertz (Klatter's own)" : "\(rate) hertz")
                            .tag(rate)
                    }
                }
                NumberField(title: "Volume, percent", value: $model.settings.volume,
                            range: KlatterSettings.volumeRange, step: 10, unit: "percent")
            }

            Section("Voice quality") {
                ForEach(KlatterParameter.all.filter { $0.code != "fl" }) { parameter in
                    ParameterRow(parameter: parameter, settings: $model.settings)
                }
                Button("Reset Voice Quality") {
                    model.settings.resetQuality()
                    announce("Voice quality is back to Klatter's own")
                }
                .disabled(model.settings.quality.isEmpty)
            }

            Section("Test") {
                TextField("Text to speak", text: $model.testText, axis: .vertical)
                    .lineLimit(3...6)
                #if os(macOS)
                HStack {
                    Button("Speak") { model.speakDirectly() }
                        .keyboardShortcut(.defaultAction)
                    Button("Speak Through \(platformName)") { model.speakThroughSystem() }
                    Button("Stop") { model.stop() }
                        .keyboardShortcut(.cancelAction)
                    Button("Save as WAV…") { model.saveWAV() }
                        .keyboardShortcut("s", modifiers: .command)
                        .disabled(model.isSaving)
                    Spacer()
                    Button("Refresh System Voices") { model.refreshSystemVoices() }
                }
                #else
                // One button to a row: a row with several would press them all.
                Button("Speak") { model.speakDirectly() }
                Button("Speak Through \(platformName)") { model.speakThroughSystem() }
                Button("Stop") { model.stop() }
                Button("Save as WAV") { model.saveWAV() }
                    .disabled(model.isSaving)
                Button("Refresh System Voices") { model.refreshSystemVoices() }
                #endif
                if !model.status.isEmpty {
                    Text(model.status)
                        .accessibilityAddTraits(.updatesFrequently)
                }
            }
        }
        .formStyle(.grouped)
        .onChangeOf(model.status) { status in
            guard !status.isEmpty else { return }
            announce(status)
        }
    }
}

/// A whole number typed exactly, with a stepper beside it. A value typed past
/// either end goes to the end, and VoiceOver says so.
struct NumberField: View {
    let title: String
    @Binding var value: Int
    let range: ClosedRange<Int>
    let step: Int
    let unit: String

    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        LabeledContent {
            HStack(spacing: 6) {
                TextField(title, text: $text)
                    .labelsHidden()
                    .multilineTextAlignment(.trailing)
                    .monospacedDigit()
                    .frame(width: 80)
                    .focused($focused)
                    .onSubmit(apply)
                    .onChangeOf(focused) { isFocused in if !isFocused { apply() } }
                    .accessibilityLabel(title)
                    .accessibilityHint("\(range.lowerBound) to \(range.upperBound)")
                Stepper(title, value: $value, in: range, step: step)
                    .labelsHidden()
                    .accessibilityLabel(title)
                    .accessibilityValue("\(value) \(unit)")
            }
        } label: {
            Text("\(title) (\(range.lowerBound)–\(range.upperBound))")
        }
        .onAppear { text = "\(value)" }
        .onChangeOf(value) { newValue in if !focused { text = "\(newValue)" } }
    }

    private func apply() {
        guard let typed = Int(text.filter(\.isNumber)) else {
            text = "\(value)"
            return
        }
        let clamped = range.clamped(typed)
        value = clamped
        text = "\(clamped)"
        if clamped != typed {
            announce("\(clamped) \(unit), the \(typed < clamped ? "lowest" : "highest") allowed")
        }
    }
}

/// One voice-quality setting: Klatter's own value until changed, typed
/// exactly or stepped, with Reset to go back to it.
struct ParameterRow: View {
    let parameter: KlatterParameter
    @Binding var settings: KlatterSettings

    @State private var text = ""
    @FocusState private var focused: Bool

    private var own: Double? { parameter.defaultValue }
    private var custom: Double? { settings.quality[parameter.code] }
    private var current: Double? { custom ?? own }

    private func describe(_ value: Double?) -> String {
        guard let value else { return "automatic" }
        let number = Self.format(value)
        return parameter.unit.isEmpty ? number : "\(number) \(parameter.unit)"
    }

    private func set(_ value: Double?) {
        settings.set(parameter, to: value)
    }

    /// One decimal at most, and none when it would be ".0" (55.000001 is 55).
    static func format(_ value: Double) -> String {
        let tenths = (value * 10).rounded() / 10
        return tenths == tenths.rounded() ? String(Int(tenths)) : String(format: "%.1f", tenths)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            LabeledContent {
                HStack(spacing: 6) {

                        TextField(parameter.name, text: $text, prompt: Text(own == nil ? "Automatic" : ""))
                            .labelsHidden()
                            .multilineTextAlignment(.trailing)
                            .monospacedDigit()
                            .frame(width: 80)
                            .focused($focused)
                            .onSubmit(apply)
                            .onChangeOf(focused) { isFocused in if !isFocused { apply() } }
                            .accessibilityLabel(parameter.name)
                            .accessibilityValue(describe(current))
                            .accessibilityHint("\(Int(parameter.range.lowerBound)) to \(Int(parameter.range.upperBound)). \(parameter.help)")
                        Stepper(parameter.name,
                                onIncrement: { step(+1) },
                                onDecrement: { step(-1) })
                            .labelsHidden()
                            .accessibilityLabel(parameter.name)
                            .accessibilityValue(describe(current))
                    Button("Reset") {
                        set(nil)
                        announce("\(parameter.name), Klatter's own, \(describe(own))")
                    }
                    .disabled(custom == nil)
                    .accessibilityLabel("Reset \(parameter.name)")
                }
            } label: {
                Text("\(parameter.name) (\(Int(parameter.range.lowerBound))–\(Int(parameter.range.upperBound))\(parameter.unit.isEmpty ? "" : " " + parameter.unit))")
            }
            Text("\(parameter.help) Klatter's own: \(describe(own)).\(custom != nil ? " Changed." : "")")
                .font(.caption).foregroundStyle(.secondary)
        }
        .onAppear(perform: refresh)
        .onChangeOf(current) { _ in if !focused { refresh() } }
    }

    private func refresh() {
        guard let current else { text = ""; return }
        text = Self.format(current)
    }

    private func step(_ direction: Double) {
        let start = current ?? (parameter.range.lowerBound + parameter.range.upperBound) / 2
        let next = min(parameter.range.upperBound, max(parameter.range.lowerBound, (start + direction * parameter.step).rounded()))
        set(next)
        refresh()
    }

    private func apply() {
        let typed = text.trimmingCharacters(in: .whitespaces)
        if typed.isEmpty {
            // Emptied: back to Klatter's own.
            if custom != nil { set(nil) }
            refresh()
            return
        }
        guard let value = Double(typed.filter { $0.isNumber || $0 == "." }) else { refresh(); return }
        let clamped = min(parameter.range.upperBound, max(parameter.range.lowerBound, value))
        if clamped != current { set(clamped) }
        refresh()
        if clamped != value {
            announce("\(parameter.name), \(describe(clamped)), the \(value < clamped ? "lowest" : "highest") allowed")
        }
    }
}

/// Speaks a short message through VoiceOver; the same on every system version.
@MainActor
func announce(_ message: String) {
    #if os(macOS)
    NSAccessibility.post(element: NSApp.mainWindow ?? NSApp as Any, notification: .announcementRequested,
                         userInfo: [.announcement: message, .priority: NSAccessibilityPriorityLevel.high.rawValue])
    #else
    UIAccessibility.post(notification: .announcement, argument: message)
    #endif
}

#if os(macOS)
let savedWhere = ""
let platformName = "macOS"
let voiceSettingsPath = "VoiceOver Utility, Speech, then the voice menu or Add Voice"
#else
let savedWhere = " It is in the Files app, in Klatter's folder."
let platformName = UIDevice.current.userInterfaceIdiom == .pad ? "iPadOS" : "iOS"
let voiceSettingsPath = "Settings, Accessibility, VoiceOver, Speech, Voice"
#endif

extension View {
    /// onChange with the new value, on macOS 13 and iOS 16 and on newer alike.
    @ViewBuilder
    func onChangeOf<V: Equatable>(_ value: V, perform action: @escaping (V) -> Void) -> some View {
        if #available(macOS 14.0, iOS 17.0, *) {
            onChange(of: value) { _, newValue in action(newValue) }
        } else {
            onChange(of: value, perform: action)
        }
    }
}
