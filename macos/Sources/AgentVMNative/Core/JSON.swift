import Foundation

enum JSON: Codable, Equatable, Sendable {
    case object([String: JSON])
    case array([JSON])
    case string(String)
    case number(Double)
    case bool(Bool)
    case null

    init(_ value: String) { self = .string(value) }
    init(_ value: Int) { self = .number(Double(value)) }
    init(_ value: Double) { self = .number(value) }
    init(_ value: Bool) { self = .bool(value) }

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let bool = try? value.decode(Bool.self) { self = .bool(bool) }
        else if let number = try? value.decode(Double.self) { self = .number(number) }
        else if let text = try? value.decode(String.self) { self = .string(text) }
        else if let items = try? value.decode([JSON].self) { self = .array(items) }
        else { self = .object(try value.decode([String: JSON].self)) }
    }

    func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case .object(let fields): try value.encode(fields)
        case .array(let items): try value.encode(items)
        case .string(let text): try value.encode(text)
        case .number(let number): try value.encode(number)
        case .bool(let bool): try value.encode(bool)
        case .null: try value.encodeNil()
        }
    }

    subscript(key: String) -> JSON {
        get { fields[key] ?? .null }
        set {
            var updated = fields
            updated[key] = newValue
            self = .object(updated)
        }
    }

    var text: String { if case .string(let value) = self { return value }; return "" }
    var number: Double { if case .number(let value) = self { return value }; return 0 }
    var integer: Int { Int(exactly: number.rounded(.towardZero)) ?? 0 }
    var flag: Bool { if case .bool(let value) = self { return value }; return false }
    var items: [JSON] { if case .array(let value) = self { return value }; return [] }
    var fields: [String: JSON] { if case .object(let value) = self { return value }; return [:] }
    var isNull: Bool { self == .null }

    var pretty: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return (try? encoder.encode(self)).flatMap { String(data: $0, encoding: .utf8) } ?? ""
    }
}
