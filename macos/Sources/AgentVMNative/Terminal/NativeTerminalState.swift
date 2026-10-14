enum NativeTerminalCommand: Equatable, Sendable {
    case theme(String)
    case paste(String)
    case pasteRejected
}

// WebKit can finish loading after a user changes appearance or pastes. Keep only the latest
// appearance and retain explicit pastes until the renderer can receive them.
struct NativeTerminalState: Sendable {
    private var ready = false
    private var theme = "light"
    private var pending: [String] = []
    private var pendingBytes = 0
    private let limit = 1 << 20

    mutating func rendererReset() { ready = false }

    mutating func setTheme(_ value: String) -> [NativeTerminalCommand] {
        let normalized = value == "dark" ? "dark" : "light"
        guard normalized != theme else { return [] }
        theme = normalized
        return ready ? [.theme(theme)] : []
    }

    mutating func paste(_ text: String) -> [NativeTerminalCommand] {
        guard !text.isEmpty else { return [] }
        if ready { return [.paste(text)] }
        let bytes = text.utf8.count
        guard bytes <= limit - pendingBytes else { return [.pasteRejected] }
        pending.append(text)
        pendingBytes += bytes
        return []
    }

    mutating func rendererReady() -> [NativeTerminalCommand] {
        guard !ready else { return [] }
        ready = true
        let commands = [.theme(theme)] + pending.map(NativeTerminalCommand.paste)
        pending.removeAll()
        pendingBytes = 0
        return commands
    }
}
