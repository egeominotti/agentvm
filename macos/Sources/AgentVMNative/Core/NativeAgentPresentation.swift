import Foundation

enum NativeAgentPresentation {
    static func name(_ task: JSON, catalog: JSON = .null) -> String {
        NativeAgentPolicy.displayName(task["agent"], catalog: catalog)
    }

    static func terminalLabel(_ session: String, task: JSON) -> String {
        session == "claude" && !showsClaudeAnalytics(task) ? name(task) : NativeMachineLogic.shellLabel(session)
    }

    static func showsClaudeAnalytics(_ task: JSON) -> Bool { NativeAgentPolicy.kind(task["agent"]) == "claude" }

    static func version(_ task: JSON) -> String {
        let selected = task["agent"]["version"].text
        if !selected.isEmpty { return selected }
        if !showsClaudeAnalytics(task) { return "CLI default" }
        return task["claude_version"].text.isEmpty ? "Image version" : task["claude_version"].text
    }

    static func model(_ task: JSON) -> String {
        let value = task["model"].text
        if !showsClaudeAnalytics(task) { return value.isEmpty || value == "default" ? "CLI default" : value }
        return NativeSettingsPolicy.models.first { $0.0 == value }?.1 ?? value
    }
}
