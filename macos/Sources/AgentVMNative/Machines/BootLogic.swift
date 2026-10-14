import Foundation

struct NativeBootStep: Identifiable {
    let label: String
    let done: Bool
    var detail = ""
    var failed = false
    var id: String { label }
}

enum NativeBootLogic {
    static func setupFailed(_ task: JSON) -> Bool { task["boot_log"].items.contains { $0.text.contains("] setup failed") } }

    static func steps(_ task: JSON) -> [NativeBootStep] {
        let logs = task["boot_log"].items.map(\.text)
        let guest = logs.compactMap { line -> (Double, String)? in
            guard line.hasPrefix("["), let marker = line.range(of: "s] "), let seconds = Double(line[line.index(after: line.startIndex)..<marker.lowerBound]) else { return nil }
            return (seconds, String(line[marker.upperBound...]))
        }
        func event(_ prefix: String) -> (Double, String)? { guest.first { $0.1.hasPrefix(prefix) } }
        func timing(_ event: (Double, String)?) -> String { event.map { String(format: "%.1f s", $0.0) } ?? "" }
        let disk = logs.first { $0.hasPrefix("host: disk ready in ") }
        var steps = [
            NativeBootStep(label: "VM slot reserved", done: NativeMachineLogic.state(task) != "queued",
                           detail: NativeMachineLogic.state(task) == "queued" ? "Waiting for a free slot" : ""),
            NativeBootStep(label: "Disk cloned from the image", done: disk != nil, detail: disk?.replacingOccurrences(of: "host: disk ready in ", with: "") ?? ""),
            NativeBootStep(label: "Debian booted", done: event("job start") != nil, detail: timing(event("job start"))),
            NativeBootStep(label: "Repository checked out", done: event("repo ready") != nil, detail: timing(event("repo ready"))),
            NativeBootStep(label: "Network up", done: event("network ready") != nil, detail: timing(event("network ready")))
        ]
        if event("running .agentvm/setup.sh") != nil {
            let end = event("setup done") ?? event("setup failed")
            let failed = end?.1.hasPrefix("setup failed") ?? false
            steps.append(NativeBootStep(label: "Repository setup (.agentvm/setup.sh)", done: end != nil,
                detail: failed ? "Failed; see setup.log" : end == nil ? "Running" : timing(end), failed: failed))
        }
        if let install = event("installing Claude Code") {
            let installed = event("Claude Code ")
            steps.append(NativeBootStep(label: install.1.replacingOccurrences(of: "installing", with: "Installing"),
                done: installed != nil, detail: timing(installed)))
        }
        let ready = event(task["interactive"].flag ? "terminal ready" : "network ready")
        steps.append(NativeBootStep(label: task["interactive"].flag ? "Claude Code ready" : "Agent started",
            done: task["ready"].flag || ready != nil, detail: timing(ready)))
        return steps
    }
}
