// Klatter as a system speech synthesizer, on macOS, iPhone and iPad.
//
// An AVSpeechSynthesisProviderAudioUnit is how macOS lets a third party add
// voices to the system: VoiceOver (VoiceOver Utility > Speech > Add Voice),
// Spoken Content, and every app that speaks. The system hands over SSML and
// pulls audio out of the render block until the unit says it is done.

import AVFoundation
import AudioToolbox
import os

public final class KlatterSpeechAudioUnit: AVSpeechSynthesisProviderAudioUnit {

    private static let log = Logger(subsystem: "com.dgomeze42.klatter", category: "audiounit")

    private var outputBus: AUAudioUnitBus
    private var busArray: AUAudioUnitBusArray!

    /// Memory for a host that passes buffers with null pointers.
    private final class Scratch: @unchecked Sendable {
        var pointer: UnsafeMutablePointer<Float>?
        var capacity = 0
        func allocate(_ frames: Int) {
            release()
            pointer = .allocate(capacity: frames)
            pointer?.initialize(repeating: 0, count: frames)
            capacity = frames
        }
        func release() {
            pointer?.deallocate()
            pointer = nil
            capacity = 0
        }
        deinit { release() }
    }

    private let scratch = Scratch()
    private let active = OSAllocatedUnfairLock<KlatterSpeaker?>(initialState: nil)

    public override init(componentDescription: AudioComponentDescription,
                         options: AudioComponentInstantiationOptions = []) throws {
        // The engine speaks at the rate chosen in the app (16000 Hz unless
        // changed), and the speaker converts that to this bus's rate, 48 kHz,
        // so any chosen rate up to 48 kHz keeps all of its bandwidth.
        let format = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 1)!
        outputBus = try AUAudioUnitBus(format: format)
        outputBus.maximumChannelCount = 1
        try super.init(componentDescription: componentDescription, options: options)
        busArray = AUAudioUnitBusArray(audioUnit: self, busType: .output, busses: [outputBus])
        maximumFramesToRender = 4096
    }

    public override var outputBusses: AUAudioUnitBusArray { busArray }

    public override func allocateRenderResources() throws {
        try super.allocateRenderResources()
        scratch.allocate(Int(maximumFramesToRender))
    }

    public override func deallocateRenderResources() {
        active.withLock { speaker in
            speaker?.cancel()
            speaker = nil
        }
        scratch.release()
        super.deallocateRenderResources()
    }

    // MARK: - Voices

    /// Klatter has one voice, whose identifier never changes, so it stays
    /// chosen in VoiceOver across updates.
    static let voiceIdentifier = "com.dgomeze42.klatter.voice.klatter"

    public override var speechVoices: [AVSpeechSynthesisProviderVoice] {
        get {
            let v = AVSpeechSynthesisProviderVoice(name: "Klatter",
                                                   identifier: Self.voiceIdentifier,
                                                   primaryLanguages: ["en-US"],
                                                   supportedLanguages: ["en-US"])
            v.gender = .male
            v.version = "0.2.0"
            return [v]
        }
        set {}
    }

    // MARK: - Speaking

    public override func synthesizeSpeechRequest(_ request: AVSpeechSynthesisProviderRequest) {
        let settings = KlatterSettingsStore.shared.current
        let items = SpeechMarkup.parse(request.ssmlRepresentation)
        let speaker = KlatterSpeaker(outputRate: outputBus.format.sampleRate)
        // Never the text itself: with VoiceOver, that is everything on screen.
        Self.log.debug("request at \(self.outputBus.format.sampleRate, privacy: .public) Hz")
        active.withLock { current in
            current?.cancel()
            current = speaker
        }
        speaker.start(items: items, settings: settings)

        // One marker for the whole utterance, at its start.
        let spoken = items.reduce(into: "") { text, item in
            if case .text(let span) = item { text += span.text }
        }
        let marker = AVSpeechSynthesisMarker(markerType: .sentence,
                                             forTextRange: NSRange(location: 0, length: (spoken as NSString).length),
                                             atByteSampleOffset: 0)
        speechSynthesisOutputMetadataBlock?([marker], request)
    }

    public override func cancelSpeechRequest() {
        active.withLock { speaker in
            speaker?.cancel()
            speaker = nil
        }
    }

    // MARK: - Rendering

    public override var internalRenderBlock: AUInternalRenderBlock {
        let box = active
        let scratch = self.scratch
        return { flags, _, frameCount, _, outputData, _, _ in
            let buffers = UnsafeMutableAudioBufferListPointer(outputData)
            let frames = Int(frameCount)
            if buffers[0].mData == nil {
                guard let memory = scratch.pointer, scratch.capacity >= frames else {
                    return kAudioUnitErr_Uninitialized
                }
                buffers[0].mData = UnsafeMutableRawPointer(memory)
            }
            buffers[0].mNumberChannels = 1
            buffers[0].mDataByteSize = UInt32(frames * MemoryLayout<Float>.size)
            guard let out = buffers[0].mData?.assumingMemoryBound(to: Float.self) else {
                return kAudioUnitErr_InvalidParameter
            }
            guard let speaker = box.withLockIfAvailable({ $0 }) ?? nil else {
                out.update(repeating: 0, count: frames)
                return noErr
            }
            if speaker.render(into: out, frames: frames, waitForAudio: true) {
                flags.pointee = .offlineUnitRenderAction_Complete
            }
            return noErr
        }
    }
}
