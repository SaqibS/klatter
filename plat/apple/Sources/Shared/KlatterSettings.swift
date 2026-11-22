// Settings shared by the Klatter app, which edits them, and the speech
// synthesis extension, which speaks with them.
//
// They are two processes, and the extension is sandboxed, so the settings live
// in a file in the App Group container both are entitled to. The extension
// reads it before every utterance, re-parsing only when the file has changed.
//
// As in the NVDA add-on: an exact rate in words per minute (30 to 1920, 219
// natural), an optional exact base pitch in hertz (40 to 400, 100 natural),
// the 20 sample rates, and every voice-quality setting, flutter off.

import Foundation
import Security

public struct KlatterSettings: Codable, Equatable, Sendable {

    public static let wpmRange = 30...1920
    public static let pitchRange = 40...400
    public static let inflectionRange = 0...200
    public static let volumeRange = 0...200
    public static let sampleRates = [
        8000, 10000, 11000, 11025, 12000, 16000, 20000, 22000, 22050, 24000,
        32000, 40000, 44000, 44100, 48000, 64000, 80000, 88000, 88200, 96000,
    ]
    /// Klatter's own rate.
    public static let defaultSampleRate = 16000
    public static let naturalWPM = 219
    public static let naturalPitch = 100

    public var sampleRate: Int = KlatterSettings.defaultSampleRate
    /// The speaking rate in words per minute.
    public var rateWPM: Int = KlatterSettings.naturalWPM
    /// Let VoiceOver's rate, and an app's, scale `rateWPM`.
    public var followSystemRate: Bool = true
    /// Speak at `pitchHz` rather than at Klatter's natural pitch.
    public var useExactPitch: Bool = false
    public var pitchHz: Int = KlatterSettings.naturalPitch
    /// Let VoiceOver's pitch, and an app's, move the pitch.
    public var followSystemPitch: Bool = true
    /// Pitch movement, percent of natural: 0 is monotone.
    public var inflection: Int = 100
    /// Volume, percent: 100 is full level.
    public var volume: Int = 100
    /// Voice-quality settings the user changed, by code ("fl", "hs", ...);
    /// everything else is the engine's default (flutter off).
    public var quality: [String: Double] = [:]

    public init() {}

    public func value(of parameter: KlatterParameter) -> Double {
        quality[parameter.code] ?? parameter.defaultValue
    }

    /// Sets a setting, or with nil puts it back to the default.
    public mutating func set(_ parameter: KlatterParameter, to value: Double?) {
        if let value, value != parameter.defaultValue {
            quality[parameter.code] = min(parameter.range.upperBound, max(parameter.range.lowerBound, value))
        } else {
            quality[parameter.code] = nil
        }
    }

    public mutating func resetQuality() { quality = [:] }

    /// Every key optional, so a file from an older version still loads.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let d = KlatterSettings()
        sampleRate = try c.decodeIfPresent(Int.self, forKey: .sampleRate) ?? d.sampleRate
        rateWPM = try c.decodeIfPresent(Int.self, forKey: .rateWPM) ?? d.rateWPM
        followSystemRate = try c.decodeIfPresent(Bool.self, forKey: .followSystemRate) ?? d.followSystemRate
        useExactPitch = try c.decodeIfPresent(Bool.self, forKey: .useExactPitch) ?? d.useExactPitch
        pitchHz = try c.decodeIfPresent(Int.self, forKey: .pitchHz) ?? d.pitchHz
        followSystemPitch = try c.decodeIfPresent(Bool.self, forKey: .followSystemPitch) ?? d.followSystemPitch
        inflection = try c.decodeIfPresent(Int.self, forKey: .inflection) ?? d.inflection
        volume = try c.decodeIfPresent(Int.self, forKey: .volume) ?? d.volume
        quality = (try? c.decodeIfPresent([String: Double].self, forKey: .quality)) ?? [:]
        clamp()
    }

    /// Brings every value into its range, and drops unknown settings.
    public mutating func clamp() {
        rateWPM = Self.wpmRange.clamped(rateWPM)
        pitchHz = Self.pitchRange.clamped(pitchHz)
        inflection = Self.inflectionRange.clamped(inflection)
        volume = Self.volumeRange.clamped(volume)
        if !Self.sampleRates.contains(sampleRate) { sampleRate = Self.defaultSampleRate }
        var kept: [String: Double] = [:]
        for (code, value) in quality {
            guard let p = KlatterParameter.named(code), value.isFinite else { continue }
            kept[code] = min(p.range.upperBound, max(p.range.lowerBound, value))
        }
        quality = kept
    }
}

extension ClosedRange where Bound == Int {
    public func clamped(_ value: Int) -> Int { Swift.min(upperBound, Swift.max(lowerBound, value)) }
}

public enum KlatterStorage {

    /// The App Group, from Info.plist (set from the KLATTER_APP_GROUP build
    /// setting): it must start with the signing team's identifier.
    public static let appGroup: String =
        (Bundle.main.object(forInfoDictionaryKey: "KlatterAppGroup") as? String) ?? ""

    /// The group container. A build with no team identifier (ad-hoc signed) is
    /// never let into a team-prefixed group container, so it uses
    /// ~/Library/Application Support/Klatter instead, which the entitlements
    /// open to the sandboxed extension.
    public static var directory: URL {
        if hasTeamIdentifier, !appGroup.isEmpty,
           let url = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup) {
            return url
        }
        #if os(macOS)
        var home = NSHomeDirectory()
        if let pw = getpwuid(getuid()) { home = String(cString: pw.pointee.pw_dir) }
        return URL(fileURLWithPath: home).appendingPathComponent("Library/Application Support/Klatter", isDirectory: true)
        #else
        // Without the group (a build signed without it), each side keeps its own.
        return URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support/Klatter", isDirectory: true)
        #endif
    }

    public static var settingsURL: URL { directory.appendingPathComponent("settings.json") }

    /// Whether this process is signed with a team identifier. On iPhone and
    /// iPad every installed app is, and the group container is simply there or not.
    #if os(iOS)
    public static let hasTeamIdentifier = true
    #else
    public static let hasTeamIdentifier: Bool = {
        var code: SecCode?
        guard SecCodeCopySelf([], &code) == errSecSuccess, let code else { return false }
        var staticCode: SecStaticCode?
        guard SecCodeCopyStaticCode(code, [], &staticCode) == errSecSuccess, let staticCode else { return false }
        var info: CFDictionary?
        guard SecCodeCopySigningInformation(staticCode, SecCSFlags(rawValue: kSecCSSigningInformation), &info) == errSecSuccess,
              let dict = info as? [String: Any] else { return false }
        return !((dict[kSecCodeInfoTeamIdentifier as String] as? String) ?? "").isEmpty
    }()
    #endif
}

/// Loads and saves the settings file. Cheap enough to consult for every
/// utterance: it re-reads only when the file's modification date moves.
public final class KlatterSettingsStore: @unchecked Sendable {

    public static let shared = KlatterSettingsStore()

    private let lock = NSLock()
    private var cached = KlatterSettings()
    private var stamp: Date?
    private var loaded = false

    public var current: KlatterSettings {
        lock.lock()
        defer { lock.unlock() }
        let url = KlatterStorage.settingsURL
        let now = (try? FileManager.default.attributesOfItem(atPath: url.path)[.modificationDate]) as? Date
        if loaded && now == stamp { return cached }
        loaded = true
        stamp = now
        if let data = try? Data(contentsOf: url),
           let decoded = try? JSONDecoder().decode(KlatterSettings.self, from: data) {
            cached = decoded
        }
        return cached
    }

    public func save(_ settings: KlatterSettings) throws {
        var settings = settings
        settings.clamp()
        lock.lock()
        defer { lock.unlock() }
        try FileManager.default.createDirectory(at: KlatterStorage.directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(settings).write(to: KlatterStorage.settingsURL, options: .atomic)
        cached = settings
        loaded = true
        stamp = (try? FileManager.default.attributesOfItem(atPath: KlatterStorage.settingsURL.path)[.modificationDate]) as? Date
    }
}
