import Foundation
import Testing
@testable import AgentVMNative

@Suite("SnapshotPolicyTests")
struct SnapshotPolicyTests {
    private func snapshot(_ id: String, _ name: String, time: Double, auto: Bool = false,
                          task: String = "machine-1234") -> JSON {
        .object(["id": .string(id), "name": .string(name), "repo": .string("/code/shop"),
                 "source_task": .string(task), "created_at": .number(time), "auto": .bool(auto)])
    }

    @Test
    func testGroupsKeepInterruptedFirstAndRecoverTheMachineTitle() {
        let snapshots = [snapshot("m", "A manual save", time: 3),
                         snapshot("a", "Implement autosave, auto 10:02", time: 2, auto: true),
                         snapshot("i", "Interrupted: Implement autosave", time: 1)]
        let group = NativeSnapshotPolicy.groups(snapshots)[0]
        #expect(group.title == "Implement autosave")
        #expect(group.snapshots.map { $0["id"].text } == ["i", "m", "a"])
        #expect(NativeSnapshotPolicy.label(snapshots[1], title: group.title) == "Automatic")
    }

    @Test
    func testSearchAndKindFilterIntersectAndHiddenSelectionsAreNotBulkTargets() {
        let snapshots = [snapshot("manual", "Keep it", time: 1),
                         snapshot("auto", "Shop, before close 11:03", time: 2, auto: true)]
        let shown = NativeSnapshotPolicy.filtered(snapshots, query: " SHOP ", kind: .automatic)
        #expect(shown.map { $0["id"].text } == ["auto"])
        #expect(NativeSnapshotPolicy.selected(shown, ids: ["manual", "auto"]) == ["auto"])
    }

    @Test
    func testNewestMachineGroupComesFirstAndManualTitlesUseRepository() {
        let groups = NativeSnapshotPolicy.groups([snapshot("older", "Named by hand", time: 1),
                                                   snapshot("new", "Another name", time: 5, task: "other-9876")])
        #expect(groups.map(\.id) == ["other-9876", "machine-1234"])
        #expect(groups[0].title == "shop #9876")
    }
}
