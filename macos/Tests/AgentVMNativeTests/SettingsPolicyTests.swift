import Foundation
import Testing
@testable import AgentVMNative

@Suite("SettingsPolicyTests")
struct SettingsPolicyTests {
    @Test
    func testMemoryChoicesRespectHostReserveAndKeepTheCurrentCustomValue() {
        #expect(NativeSettingsPolicy.memoryChoices(ram: 12288, current: 3072) == [1024, 2048, 3072, 4096])
        #expect(NativeSettingsPolicy.memoryChoices(ram: 4096, current: 1024) == [1024])
    }

    @Test
    func testS3PresetPreservesUserBucketAndAccessKey() {
        let original = JSON.object(["bucket": .string("my-backups"), "access_key": .string("existing"),
                                    "prefix": .string("archive")])
        let local = NativeSettingsPolicy.preset("local", applyingTo: original)
        #expect(local["bucket"].text == "my-backups")
        #expect(local["access_key"].text == "existing")
        #expect(local["prefix"].text == "archive")
        #expect(local["endpoint"].text == "http://127.0.0.1:9100")
        #expect(local["path_style"].flag)
        #expect(local["secret"].isNull)
    }

    @Test
    func testSearchRequiresAllWordsAcrossTitleAndKeywords() {
        #expect(NativeSettingsPolicy.sections(matching: "private TOKEN").map(\.id) == ["git"])
        #expect(NativeSettingsPolicy.sections(matching: "   ").count == 14)
        #expect(NativeSettingsPolicy.sections(matching: "memory bucket").isEmpty)
    }

    @Test
    func testInferenceProvidersAndCustomCommandsAreDiscoverableWithoutHidingClaudeAccount() {
        #expect(NativeSettingsPolicy.sections(matching: "inference credentials").map(\.id) == ["providers"])
        #expect(NativeSettingsPolicy.sections(matching: "custom install").map(\.id) == ["custom_cli"])
        #expect(NativeSettingsPolicy.sections(matching: "MCP setup").map(\.id) == ["mcp"])
        #expect(NativeSettingsPolicy.catalog.contains { $0.id == "account" && $0.title == "Claude account" })
    }

    @Test
    func testDefaultCodingCLISearchFindsTheGeneralPage() {
        #expect(NativeSettingsPolicy.sections(matching: "default CLI").map(\.id) == ["general"])
        #expect(NativeSettingsPolicy.sections(matching: "coding agent").map(\.id) == ["general"])
    }
}
