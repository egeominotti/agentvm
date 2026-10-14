import Foundation

struct NativeLaunchCapacity {
    let freeMB: Int
    let fit: Int
    let starts: Int
    let waitingFor: String?
}

enum NativeLauncherLogic {
    static func capacity(status: JSON, memory: Int) -> NativeLaunchCapacity? {
        let ram = status["host"]["ram_mb"].integer
        guard memory > 0, ram > 0, !status["ram_committed_mb"].isNull,
              !status["running"].isNull, !status["concurrency"].isNull else { return nil }
        // Use the same macOS reserve and committed memory budget as the dashboard.
        let budget = max(0, ram - 8192)
        let free = budget - min(budget, max(0, status["ram_committed_mb"].integer))
        let concurrency = max(0, status["concurrency"].integer)
        let slots = concurrency - min(concurrency, max(0, status["running"].integer))
        let fit = free / memory
        let starts = min(fit, slots)
        let waiting = starts == 0 ? (slots < fit ? "concurrency" : "memory") : nil
        return NativeLaunchCapacity(freeMB: free, fit: fit, starts: starts, waitingFor: waiting)
    }

    static func repoReference(_ check: JSON) -> String {
        let branch = check["branch"].text
        let sha = String(check["sha"].text.prefix(7))
        let reference = branch.isEmpty ? (check["to_clone"].flag ? "default branch" : "detached HEAD") : branch
        return sha.isEmpty ? reference : "\(reference) · \(sha)"
    }

    static func blocker(status: JSON, connected: Bool, repo: String, check: JSON, interactive: Bool, prompt: String) -> String? {
        if !connected { return "Connecting to agentvm…" }
        if !status["golden"].flag { return "Build the VM image in Settings → VM image." }
        if !status["token"].flag { return "Connect your Claude account in Settings → Claude account." }
        if repo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return "Choose a repository." }
        if check.isNull { return "Checking the repository…" }
        if !check["ok"].flag { return check["error"].text.isEmpty ? "Choose a git repository with at least one commit." : check["error"].text }
        if !interactive && prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return "Write a task for Claude." }
        return nil
    }

    static func memoryChoices(host: Int, current: Int) -> [Int] {
        let maximum = max(host - 8192, 1024)
        let usual = [1024, 2048, 4096, 6144, 8192, 12288, 16384].filter { $0 <= maximum }
        return Array(Set(usual + [current])).sorted()
    }

    static func recent(_ paths: [String]) -> [String] {
        var result: [String] = []
        for path in paths {
            let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty && !result.contains(trimmed) { result.append(trimmed) }
        }
        return Array(result.prefix(8))
    }
}
