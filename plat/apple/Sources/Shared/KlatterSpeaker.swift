// Speaking with the Klatter engine.
//
// `KlatterSynthesis.run` does the speaking: it turns the settings into
// Klatter's voice (words per minute, pitch in hertz, inflection, volume) and
// voice-quality array, and pulls 16-bit samples from the engine at the
// settings' sample rate. The live speaker and the WAV export both use it, so a
// saved file sounds exactly as the voice does.

import AVFoundation
import CKlatter
import Foundation

public enum KlatterSynthesis {

    /// Receives samples at the engine's rate; returns false to stop at once.
    public typealias Sink = (UnsafeBufferPointer<Int16>) -> Bool

    /// Speaks `items` with `settings` at `settings.sampleRate`. Pauses come to
    /// the sink as silence. Returns false if the sink stopped it.
    @discardableResult
    public static func run(items: [SpeechItem], settings: KlatterSettings, sink: @escaping Sink) -> Bool {
        let rate = settings.sampleRate
        let quality = KlatterParameter.array(for: settings.quality)
        var block = KlatterPullBlock()
        var buffer = [Int16](repeating: 0, count: max(256, rate / 20))
        for item in items {
            switch item {
            case .pause(let seconds):
                let silence = [Int16](repeating: 0, count: Int(seconds * Double(rate)))
                if !silence.withUnsafeBufferPointer(sink) { return false }
            case .text(let span):
                var text = span.text
                // Klatter has no spelling mode: letters apart are read one by one.
                if span.spelled { text = text.map(String.init).joined(separator: " ") }
                guard text.contains(where: { !$0.isWhitespace }) else { continue }
                let scale = settings.followSystemRate ? span.rate : 1
                let wpm = Double(KlatterSettings.wpmRange.clamped(Int((Double(settings.rateWPM) * scale).rounded())))
                let basePitch = Double(settings.useExactPitch ? settings.pitchHz : KlatterSettings.naturalPitch)
                let pitch = min(400, max(40, basePitch * (settings.followSystemPitch ? span.pitch : 1)))
                var voice = KlatterVoice(wpm: wpm, pitch_hz: pitch,
                                         inflection: Double(settings.inflection) / 100,
                                         volume: min(2, max(0, Double(settings.volume) / 100 * span.volume)))
                let utterance: OpaquePointer? = text.withCString { cText in
                    var texts: [UnsafePointer<CChar>?] = [cText]
                    return quality.withUnsafeBufferPointer { q in
                        klatter_utterance_create_quality(&texts, nil, 1, UInt32(rate), &voice,
                                                         q.baseAddress, q.count, nil)
                    }
                }
                guard let utterance else { continue }
                defer { klatter_utterance_free(utterance) }
                while true {
                    let status = buffer.withUnsafeMutableBufferPointer { out in
                        klatter_utterance_read(utterance, out.baseAddress, out.count, &block)
                    }
                    if status < 0 { break }
                    let n = Int(block.sample_count)
                    if n > 0, !buffer.withUnsafeBufferPointer({ sink(UnsafeBufferPointer(rebasing: $0[0..<n])) }) {
                        return false
                    }
                    if status == 0 { break }
                }
            }
        }
        return true
    }

    /// The text box's text as speech items.
    public static func items(forPlainText text: String) -> [SpeechItem] {
        let escaped = text.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
        return SpeechMarkup.parse("<speak>\(escaped)</speak>")
    }

    /// Writes `items` to a 16-bit mono WAV file at the settings' sample rate.
    /// Returns how many seconds it holds.
    public static func writeWAV(items: [SpeechItem], settings: KlatterSettings, to url: URL,
                                shouldStop: @escaping () -> Bool = { false }) throws -> Double {
        var samples = [Int16]()
        run(items: items, settings: settings) { chunk in
            samples.append(contentsOf: chunk)
            return !shouldStop()
        }
        let rate = UInt32(settings.sampleRate)
        let dataBytes = UInt32(samples.count * 2)
        var data = Data(capacity: 44 + Int(dataBytes))
        func put32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { data.append(contentsOf: $0) } }
        func put16(_ v: UInt16) { withUnsafeBytes(of: v.littleEndian) { data.append(contentsOf: $0) } }
        data.append(contentsOf: Array("RIFF".utf8)); put32(36 + dataBytes)
        data.append(contentsOf: Array("WAVE".utf8))
        data.append(contentsOf: Array("fmt ".utf8)); put32(16)
        put16(1); put16(1); put32(rate); put32(rate * 2); put16(2); put16(16)
        data.append(contentsOf: Array("data".utf8)); put32(dataBytes)
        for s in samples { put16(UInt16(bitPattern: s)) }
        try data.write(to: url, options: .atomic)
        return Double(samples.count) / Double(rate)
    }
}

/// One utterance being spoken live: the engine runs on its own thread, faster
/// than real time, and fills a buffer that a render callback empties. When the
/// engine's rate differs from the output's, the audio is converted on the way.
public final class KlatterSpeaker: @unchecked Sendable {

    public let outputRate: Double

    private let lock = NSCondition()
    private var buffer: [Float] = []
    private var readIndex = 0
    private var finished = false
    private var cancelled = false
    /// How far ahead of playback the engine may get before it waits.
    private let aheadLimit: Int

    public init(outputRate: Double) {
        self.outputRate = outputRate
        aheadLimit = Int(outputRate * 8)
    }

    /// Starts speaking `items` on a background thread.
    public func start(items: [SpeechItem], settings: KlatterSettings) {
        let thread = Thread { [self] in
            self.produce(items: items, settings: settings)
        }
        thread.qualityOfService = .userInteractive
        thread.name = "Klatter"
        thread.start()
    }

    public func cancel() {
        lock.lock()
        cancelled = true
        lock.broadcast()
        lock.unlock()
    }

    /// Copies up to `frames` samples into `out`, padding with silence. Returns
    /// whether the utterance has been played to the end.
    ///
    /// `waitForAudio`: the system renders a speech extension as fast as it
    /// will go and plays what it collected afterwards, not against a clock, so
    /// silence handed over while the engine is still working is not a glitch
    /// covered up but a gap put into the speech. There, wait for the engine
    /// (up to a few seconds) instead. A live audio device must never wait.
    public func render(into out: UnsafeMutablePointer<Float>, frames: Int, waitForAudio: Bool = false) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if waitForAudio {
            let deadline = Date().addingTimeInterval(5)
            while buffer.count - readIndex < frames && !finished && !cancelled {
                if !lock.wait(until: deadline) { break }
            }
        }
        let n = min(frames, buffer.count - readIndex)
        if n > 0 {
            buffer.withUnsafeBufferPointer { src in
                out.update(from: src.baseAddress! + readIndex, count: n)
            }
            readIndex += n
            // Drop what has been played now and then, so a long text does not
            // keep all its audio.
            if readIndex > 1 << 16 && readIndex * 2 > buffer.count {
                buffer.removeFirst(readIndex)
                readIndex = 0
            }
            lock.broadcast()
        }
        if n < frames { (out + n).update(repeating: 0, count: frames - n) }
        return (finished || cancelled) && readIndex >= buffer.count
    }

    // MARK: - The engine's side

    /// Adds converted samples; waits while far enough ahead. False once cancelled.
    private func push(_ samples: UnsafeBufferPointer<Float>) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        while !cancelled && buffer.count - readIndex > aheadLimit { lock.wait() }
        if cancelled { return false }
        buffer.append(contentsOf: samples)
        lock.broadcast()
        return true
    }

    private func produce(items: [SpeechItem], settings: KlatterSettings) {
        defer {
            lock.lock()
            finished = true
            lock.broadcast()
            lock.unlock()
        }
        let engineRate = Double(settings.sampleRate)
        let resampler = engineRate == outputRate ? nil : Resampler(from: engineRate, to: outputRate)
        var floats = [Float]()
        KlatterSynthesis.run(items: items, settings: settings) { [self] chunk in
            floats.removeAll(keepingCapacity: true)
            floats.reserveCapacity(chunk.count)
            for s in chunk { floats.append(Float(s) / 32768) }
            if let resampler {
                return floats.withUnsafeBufferPointer { resampler.convert($0, last: false, into: push) }
            }
            return floats.withUnsafeBufferPointer(push)
        }
        if let resampler { _ = resampler.convert(UnsafeBufferPointer(start: nil, count: 0), last: true, into: push) }
    }
}

/// Streams mono audio from one rate to another through AVAudioConverter, at
/// its best quality.
final class Resampler {
    private let converter: AVAudioConverter
    private let inFormat: AVAudioFormat
    private let outFormat: AVAudioFormat
    private let ratio: Double

    init?(from: Double, to: Double) {
        guard let i = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: from, channels: 1, interleaved: false),
              let o = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: to, channels: 1, interleaved: false),
              let c = AVAudioConverter(from: i, to: o) else { return nil }
        c.sampleRateConverterQuality = AVAudioQuality.max.rawValue
        c.sampleRateConverterAlgorithm = AVSampleRateConverterAlgorithm_Mastering
        converter = c
        inFormat = i
        outFormat = o
        ratio = to / from
    }

    /// Converts `input` (and, when `last`, flushes what the converter holds),
    /// handing the output to `sink`. False if the sink refused it.
    func convert(_ input: UnsafeBufferPointer<Float>, last: Bool,
                 into sink: (UnsafeBufferPointer<Float>) -> Bool) -> Bool {
        var pending: AVAudioPCMBuffer?
        if input.count > 0, let b = AVAudioPCMBuffer(pcmFormat: inFormat, frameCapacity: AVAudioFrameCount(input.count)) {
            b.frameLength = AVAudioFrameCount(input.count)
            b.floatChannelData![0].update(from: input.baseAddress!, count: input.count)
            pending = b
        }
        let capacity = AVAudioFrameCount(Double(max(input.count, 1024)) * ratio) + 1024
        while true {
            guard let out = AVAudioPCMBuffer(pcmFormat: outFormat, frameCapacity: capacity) else { return false }
            var error: NSError?
            let status = converter.convert(to: out, error: &error) { _, inputStatus in
                if let b = pending {
                    pending = nil
                    inputStatus.pointee = .haveData
                    return b
                }
                inputStatus.pointee = last ? .endOfStream : .noDataNow
                return nil
            }
            if out.frameLength > 0 {
                let ok = sink(UnsafeBufferPointer(start: out.floatChannelData![0], count: Int(out.frameLength)))
                if !ok { return false }
            }
            if status != .haveData { return status != .error }
        }
    }
}
