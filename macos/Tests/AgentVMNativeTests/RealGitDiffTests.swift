import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native diff paths from real Git")
struct RealGitDiffTests {
    @Test("Git-quoted UTF-8 filenames display their original path")
    func unicodePath() throws { try check(path: "café.txt") }

    @Test("Git-quoted tab filenames display their original path")
    func tabPath() throws { try check(path: "tab\tname.txt") }

    @Test("Unquoted paths containing the header separator remain complete")
    func separatorPath() throws { try check(path: "dir b/file.txt") }

    private func check(path: String) throws {
        let repo = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: repo) }
        _ = try git(["init", "-q"], repo: repo)
        let file = repo.appendingPathComponent(path)
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("old\n".utf8).write(to: file)
        _ = try git(["add", "--", path], repo: repo)
        try Data("new\n".utf8).write(to: file)
        let patch = try git(["-c", "core.quotepath=true", "diff", "--no-ext-diff", "--no-textconv", "--no-color"], repo: repo)
        let files = NativeMachineLogic.diff(patch)
        #expect(files.count == 1)
        #expect(files.first?.path == path)
        #expect(files.first?.added == 1)
        #expect(files.first?.removed == 1)
        #expect(files.first?.lines.contains("+new") == true)
        #expect(files.first?.lines.contains("-old") == true)
    }

    private func git(_ arguments: [String], repo: URL) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["--no-pager", "-C", repo.path] + arguments
        var environment = ProcessInfo.processInfo.environment
        environment["GIT_CONFIG_GLOBAL"] = "/dev/null"
        environment["GIT_CONFIG_NOSYSTEM"] = "1"
        process.environment = environment
        let output = Pipe()
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice
        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw RuntimeError.message("Real Git fixture failed: \(arguments)") }
        return String(decoding: data, as: UTF8.self)
    }
}
