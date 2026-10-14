import Testing
@testable import AgentVMNative

@Suite("Native Claude version catalog")
struct NativeClaudeVersionCatalogTests {
    @Test("Versions load only after defaults initialize with Claude selected")
    func selectedResource() {
        let catalog = NativeClaudeVersionCatalog()
        #expect(!catalog.shouldLoad(kind: "claude", initialized: false))
        #expect(!catalog.shouldLoad(kind: "codex", initialized: true))
        #expect(!catalog.shouldLoad(kind: "custom", initialized: true))
        #expect(catalog.shouldLoad(kind: "claude", initialized: true))
    }

    @Test("Successful version responses stay cached across CLI changes, including empty lists")
    func successfulCache() {
        for releases in [[], ["2.1.294", "2.1.293"]] {
            var catalog = NativeClaudeVersionCatalog()
            catalog.receive(releases)
            #expect(catalog.releases == releases)
            #expect(!catalog.shouldLoad(kind: "codex", initialized: true))
            #expect(!catalog.shouldLoad(kind: "claude", initialized: true))
        }
    }

    @Test("A failed Claude resource remains retryable and never reports errors on another CLI")
    func scopedFailure() {
        var catalog = NativeClaudeVersionCatalog()
        catalog.fail("The connection closed.")
        #expect(catalog.shouldLoad(kind: "claude", initialized: true))
        #expect(catalog.visibleError(kind: "claude") == "The connection closed.")
        #expect(catalog.visibleError(kind: "codex") == nil)
        #expect(catalog.visibleError(kind: "custom") == nil)
        catalog.receive(["2.1.294"])
        #expect(catalog.visibleError(kind: "claude") == nil)
    }
}
