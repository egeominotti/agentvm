import Foundation

struct NativeSSEEvent: Equatable, Sendable {
    let id: String
    let type: String
    let data: JSON
}

enum NativeSSERecord: Equatable, Sendable {
    case event(NativeSSEEvent)
    case comment(String)
}

struct NativeSSEDecoder: Sendable {
    private let limit: Int
    private var line: [UInt8] = []
    private var data: [String] = []
    private var dataBytes = 0
    private var id = ""
    private var type = "message"
    private var firstLine = true
    private var carriageReturn = false

    init(limit: Int = 256 << 10) { self.limit = max(1, limit) }

    mutating func append(_ byte: UInt8) throws -> NativeSSERecord? {
        if byte == 10 {
            if carriageReturn { carriageReturn = false; return nil }
            return try consumeLine()
        }
        if byte == 13 { carriageReturn = true; return try consumeLine() }
        carriageReturn = false
        guard line.count < limit else { throw oversized() }
        line.append(byte)
        return nil
    }

    private mutating func consumeLine() throws -> NativeSSERecord? {
        defer { line.removeAll(keepingCapacity: true) }
        if firstLine {
            firstLine = false
            if line.starts(with: [0xef, 0xbb, 0xbf]) { line.removeFirst(3) }
        }
        guard let text = String(bytes: line, encoding: .utf8) else {
            throw NativeAPIError(status: nil, message: "The event stream contains invalid UTF-8.")
        }
        if text.isEmpty {
            defer { data.removeAll(keepingCapacity: true); dataBytes = 0; type = "message" }
            guard !data.isEmpty else { return nil }
            let payload: JSON
            do { payload = try JSONDecoder().decode(JSON.self, from: Data(data.joined(separator: "\n").utf8)) }
            catch { throw NativeAPIError(status: nil, message: "The event stream contains invalid JSON.") }
            return .event(NativeSSEEvent(id: id, type: type, data: payload))
        }
        if text.hasPrefix(":") { return .comment(String(text.dropFirst())) }
        let pieces = text.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        var value = pieces.count == 2 ? String(pieces[1]) : ""
        if value.hasPrefix(" ") { value.removeFirst() }
        switch pieces[0] {
        case "data":
            let size = value.utf8.count + (data.isEmpty ? 0 : 1)
            guard size <= limit - dataBytes else { throw oversized() }
            data.append(value); dataBytes += size
        case "event": type = value.isEmpty ? "message" : value
        case "id": if !value.contains("\u{0}") { id = value }
        default: break
        }
        return nil
    }

    private func oversized() -> NativeAPIError {
        NativeAPIError(status: nil, message: "An event exceeds the event stream's size limit.")
    }
}
