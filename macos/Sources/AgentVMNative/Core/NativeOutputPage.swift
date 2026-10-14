import Foundation

struct NativeOutputPage: Sendable {
    let events: [NativeSSEEvent]
    let nextCursor: UInt64
    let more: Bool
    let dropped: Int

    init(_ value: JSON, cursor: UInt64) throws {
        guard case .array(let items) = value["items"], items.count <= 500,
              let next = Self.integer(value["next_cursor"]), next >= cursor,
              let omitted = Self.integer(value["dropped"]), case .bool(let more) = value["more"] else {
            throw Self.unreadable()
        }
        var events: [NativeSSEEvent] = []
        var previous = cursor
        for row in items {
            guard let seq = Self.integer(row["seq"]), seq > previous,
                  case .string(let type) = row["item"]["type"], !type.isEmpty,
                  let data = row["item"].fields["data"] else { throw Self.unreadable() }
            events.append(NativeSSEEvent(id: String(seq), type: type, data: data))
            previous = seq
        }
        guard (events.isEmpty || next == previous), (!more || (!events.isEmpty && next > cursor)) else {
            throw Self.unreadable()
        }
        self.events = events; self.nextCursor = next; self.more = more
        self.dropped = Int(clamping: omitted)
    }

    static func usesLegacyStream(_ error: Error) -> Bool { (error as? NativeAPIError)?.status == 404 }

    private static func integer(_ value: JSON) -> UInt64? {
        guard case .number(let number) = value else { return nil }
        return UInt64(exactly: number)
    }

    private static func unreadable() -> NativeAPIError {
        NativeAPIError(status: nil, message: "The server returned an invalid output page.")
    }
}
