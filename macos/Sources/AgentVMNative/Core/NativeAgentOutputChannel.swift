import Foundation

struct NativeAgentOutputSnapshot: Sendable {
    let buffer: NativeAgentOutputBuffer
    let loaded: Bool
}

actor NativeAgentOutputChannel {
    private var buffer: NativeAgentOutputBuffer
    private var loaded = false
    private var dirty = false

    init(buffer: NativeAgentOutputBuffer) {
        self.buffer = buffer
        self.buffer.beginConnection()
    }

    func receive(_ record: NativeSSERecord, ended: Bool) -> Bool {
        let first = !loaded
        loaded = true
        switch record {
        case .event(let event): dirty = buffer.append(event) || dirty || first
        case .comment:
            dirty = buffer.completeHistory() || dirty || first
            // The server's first keepalive follows its complete initial history.
            return !ended
        }
        return true
    }

    func snapshot(force: Bool = false) -> NativeAgentOutputSnapshot? {
        guard dirty || force else { return nil }
        dirty = false
        return NativeAgentOutputSnapshot(buffer: buffer, loaded: loaded)
    }
}
