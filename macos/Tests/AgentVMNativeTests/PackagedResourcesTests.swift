import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native packaged resources")
struct PackagedResourcesTests {
    @Test("Relocated apps read their embedded assets in both SwiftPM bundle layouts", arguments: ["flat", "macos"])
    func relocatedApplication(layout: String) throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: folder) }
        let application = try appBundle(in: folder)
        let embedded = application.resourceURL!.appendingPathComponent("AgentVMNative_AgentVMNative.bundle")
        let relative = layout == "flat" ? "Resources" : "Contents/Resources/Resources"
        let assets = embedded.appendingPathComponent(relative)
        try FileManager.default.createDirectory(at: assets, withIntermediateDirectories: true)
        if layout == "macos" { try metadata(at: embedded.appendingPathComponent("Contents/Info.plist"), kind: "BNDL") }
        try Data("only this relocated app's terminal".utf8).write(to: assets.appendingPathComponent("terminal.html"))
        try Data("only this relocated app's preview".utf8).write(to: assets.appendingPathComponent("preview.html"))
        let root = try #require(NativeResourceBundle.resources(in: application))
        #expect(root.path == assets.path)
        #expect(try String(contentsOf: root.appendingPathComponent("terminal.html")) == "only this relocated app's terminal")
        #expect(try String(contentsOf: root.appendingPathComponent("preview.html")) == "only this relocated app's preview")
    }

    @Test("An incomplete app never masks missing embedded resources with its build directory")
    func incompleteApplication() throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: folder) }
        let application = try appBundle(in: folder)
        #expect(NativeResourceBundle.resources(in: application) == nil)
    }

    @Test("Swift package test executables retain their actual module resources")
    func packageExecutable() throws {
        let root = try #require(NativeResourceBundle.resources(in: .main))
        #expect(root.path.hasPrefix(Bundle.module.bundleURL.path + "/"))
        #expect(try String(contentsOf: root.appendingPathComponent("terminal.html")).contains("<title>Terminal</title>"))
        #expect(try Data(contentsOf: root.appendingPathComponent("dist/terminal.js")).count > 1000)
    }

    private func appBundle(in folder: URL) throws -> Bundle {
        let app = folder.appendingPathComponent("Relocated.app", isDirectory: true)
        let contents = app.appendingPathComponent("Contents", isDirectory: true)
        try FileManager.default.createDirectory(at: contents.appendingPathComponent("Resources"), withIntermediateDirectories: true)
        try metadata(at: contents.appendingPathComponent("Info.plist"), kind: "APPL")
        return try #require(Bundle(url: app))
    }

    private func metadata(at url: URL, kind: String) throws {
        let info = ["CFBundleIdentifier": "com.agentvm.resource-test", "CFBundlePackageType": kind]
        let bytes = try PropertyListSerialization.data(fromPropertyList: info, format: .xml, options: 0)
        try bytes.write(to: url)
    }
}
