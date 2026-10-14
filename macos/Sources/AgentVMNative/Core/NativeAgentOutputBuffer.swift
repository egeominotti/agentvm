import Foundation

struct NativeAgentOutputEntry: Identifiable, Equatable, Sendable {
    let id: String
    let title: String
    let text: String
    let isError: Bool
}

struct NativeAgentOutputBuffer: Sendable {
    private var values: [NativeAgentOutputEntry] = []
    private var head = 0
    private var bytes = 0
    private var removedBytes = 0
    private let countLimit: Int
    private let byteLimit: Int
    private var replacing = false
    private var lastSequence: UInt64?
    private var finiteCursor: UInt64?
    var entries: [NativeAgentOutputEntry] { Array(values.dropFirst(head)) }
    private(set) var state = ""
    private(set) var dropped = 0

    init(countLimit: Int = 500, byteLimit: Int = 1 << 20) {
        self.countLimit = max(0, countLimit); self.byteLimit = max(0, byteLimit)
    }
    mutating func beginConnection() { replacing = true }

    mutating func append(_ page: NativeOutputPage) -> Bool {
        let reset = completeHistory()
        if let finiteCursor, page.nextCursor <= finiteCursor { return reset }
        finiteCursor = page.nextCursor
        var changed = reset
        for event in page.events { changed = append(event) || changed }
        if page.dropped > 0 { omit(page.dropped); changed = true }
        return changed
    }

    /// A comment also completes an empty history after the server has restarted.
    mutating func completeHistory() -> Bool {
        guard replacing else { return false }
        values.removeAll(); head = 0; bytes = 0; removedBytes = 0
        state = ""; dropped = 0; lastSequence = nil; finiteCursor = nil; replacing = false
        return true
    }

    mutating func append(_ event: NativeSSEEvent) -> Bool {
        let reset = completeHistory()
        if let sequence = UInt64(event.id) {
            if let lastSequence, sequence <= lastSequence { return reset }
            lastSequence = sequence
        }
        if event.type == "state" { state = event.data["state"].text; return true }
        guard event.type == "agent", let entry = entry(event) else { return reset }
        let entryBytes = size(entry)
        guard countLimit > 0, entryBytes <= byteLimit else { omit(); return true }
        values.append(entry); bytes += entryBytes
        while values.count - head > countLimit || bytes > byteLimit {
            let size = size(values[head])
            bytes -= size; removedBytes += size; head += 1; omit()
        }
        if head > 0 && (head >= 64 || removedBytes >= max(1, byteLimit / 4)) {
            values.removeFirst(head); head = 0; removedBytes = 0
        }
        return true
    }

    private func size(_ entry: NativeAgentOutputEntry) -> Int {
        entry.id.utf8.count + entry.title.utf8.count + entry.text.utf8.count
    }

    private mutating func omit(_ count: Int = 1) {
        dropped = count > Int.max - dropped ? Int.max : dropped + count
    }

    private func entry(_ event: NativeSSEEvent) -> NativeAgentOutputEntry? {
        let data = event.data
        let title: String
        let text: String
        switch data["kind"].text {
        case "init": title = "Agent initialized"; text = data["model"].text
        case "text": title = "Output"; text = data["text"].text
        case "tool_use": title = data["name"].text.isEmpty ? "Tool" : data["name"].text; text = data["summary"].text
        case "tool_result": title = data["is_error"].flag ? "Tool error" : "Tool result"; text = data["summary"].text
        case "retry": title = "Retry"; text = data["attempt"].isNull ? "" : "Attempt \(data["attempt"].integer)"
        case "result": title = data["is_error"].flag ? "Run error" : "Result"; text = data["text"].text
        case "unparsed": title = "Raw output"; text = data["raw"].text
        default: return nil
        }
        return NativeAgentOutputEntry(id: event.id.isEmpty ? UUID().uuidString : event.id,
                                      title: title, text: text, isError: data["is_error"].flag)
    }
}
