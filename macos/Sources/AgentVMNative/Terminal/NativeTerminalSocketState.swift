import Foundation

enum NativeTerminalSocketMessage: Equatable, Sendable {
    case input(Data)
    case resize(cols: Int, rows: Int)
}

struct NativeTerminalSocketState: Sendable {
    private(set) var opened = false
    private var closed = false
    private let limit: Int
    private var input: [Data] = []
    private var head = 0
    private var bytes = 0
    private var desiredSize: NativeTerminalSocketMessage?
    private var sizePending = false
    private var inputNext = false

    init(limit: Int = 1 << 20) { self.limit = max(0, limit) }
    var hasPending: Bool { head < input.count || sizePending }

    mutating func enqueueInput(_ data: Data) -> Bool {
        guard !closed, data.count <= limit - bytes else { return false }
        if !data.isEmpty { input.append(data); bytes += data.count }
        return true
    }

    mutating func resize(cols: Int, rows: Int) {
        guard !closed else { return }
        let size = NativeTerminalSocketMessage.resize(cols: cols, rows: rows)
        if desiredSize != size { desiredSize = size; sizePending = true }
    }

    mutating func connectionOpened() {
        guard !closed else { return }
        opened = true
        sizePending = desiredSize != nil
        inputNext = false
    }
    mutating func connectionClosed() { opened = false }

    mutating func close() {
        closed = true; opened = false
        input.removeAll(); head = 0; bytes = 0
        desiredSize = nil; sizePending = false
    }

    mutating func nextAttempt() -> NativeTerminalSocketMessage? {
        guard opened, !closed else { return nil }
        if sizePending, (!inputNext || head == input.count), let size = desiredSize {
            sizePending = false; inputNext = true
            return size
        }
        guard head < input.count else { return nil }
        let data = input[head]
        head += 1; bytes -= data.count; inputNext = false
        // Removing before the send prevents an ambiguous failed attempt from being replayed.
        if head == input.count { input.removeAll(keepingCapacity: true); head = 0 }
        else if head >= 64 && head >= input.count / 2 { input.removeFirst(head); head = 0 }
        return .input(data)
    }
}
