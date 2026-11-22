// Klatter's voice-quality settings, as the engine describes them
// (klatter_parameter_info): flutter, head size, breathiness, F4 to F6 and the
// rest. The app shows one control for each; the extension passes them all.

import CKlatter
import Foundation

public struct KlatterParameter: Identifiable, Hashable, Sendable {
    public let index: Int
    public let code: String
    public let name: String
    public let unit: String
    public let help: String
    public let range: ClosedRange<Double>
    public let defaultValue: Double
    public let step: Double

    public var id: String { code }

    public static let all: [KlatterParameter] = (0..<klatter_parameter_count()).compactMap { i in
        var info = KlatterParameterInfo()
        guard klatter_parameter_info(i, &info) == 1 else { return nil }
        return KlatterParameter(index: i,
                                code: String(cString: info.code),
                                name: String(cString: info.name),
                                unit: String(cString: info.unit),
                                help: String(cString: info.help),
                                range: info.min...info.max,
                                defaultValue: info.default_value,
                                step: info.step)
    }

    public static func named(_ code: String) -> KlatterParameter? { all.first { $0.code == code } }

    /// The settings as the engine takes them: every value, in order, each the
    /// user's if changed and the default otherwise.
    public static func array(for custom: [String: Double]) -> [Double] {
        all.map { p in custom[p.code].map { min(p.range.upperBound, max(p.range.lowerBound, $0)) } ?? p.defaultValue }
    }
}
