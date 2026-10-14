struct NativeClaudeVersionCatalog: Sendable {
    private(set) var releases: [String] = []
    private var error: String?
    private var loaded = false

    func shouldLoad(kind: String, initialized: Bool) -> Bool { initialized && kind == "claude" && !loaded }

    mutating func receive(_ releases: [String]) {
        self.releases = releases
        error = nil
        loaded = true
    }

    mutating func fail(_ message: String) { error = message }
    func visibleError(kind: String) -> String? { kind == "claude" ? error : nil }
}
