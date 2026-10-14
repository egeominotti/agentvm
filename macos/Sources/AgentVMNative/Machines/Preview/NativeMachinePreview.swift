import SwiftUI

struct NativeMachinePreview: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @Environment(\.scenePhase) private var phase
    @StateObject private var feed: NativePreviewFeed

    init(store: NativeStore, task: JSON) {
        self.store = store
        self.task = task
        _feed = StateObject(wrappedValue: NativePreviewFeed(api: store.api, id: task["id"].text))
    }

    private var live: Bool {
        NativePreviewPolicy.shouldStream(visible: true, active: true, running: store.connected && NativeMachineLogic.state(task) == "running",
                                        interactive: task["interactive"].flag, ready: task["ready"].flag)
    }

    var body: some View {
        Group {
            if !feed.screen.text.isEmpty {
                Canvas(rendersAsynchronously: true) { context, size in
                    let scale = min(1, size.width / (Double(feed.screen.cols) * 6.6), size.height / (Double(feed.screen.rows) * 13))
                    context.scaleBy(x: scale, y: scale)
                    context.draw(Text(feed.screen.text).font(.system(size: 11, design: .monospaced)).foregroundStyle(.primary),
                                 at: .zero, anchor: .topLeading)
                }.padding(8)
            } else {
                Text(fallback).font(.system(size: 11, design: .monospaced))
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading).padding(10)
            }
        }
        .clipped().background(Color.secondary.opacity(0.07), in: RoundedRectangle(cornerRadius: 8))
        .background(NativePreviewVisibility(changed: feed.setVisible))
        .accessibilityElement(children: .ignore).accessibilityLabel("Live Claude terminal preview")
        .accessibilityValue(feed.screen.text.isEmpty ? fallback : feed.screen.text)
        .onChange(of: phase, initial: true) { _, value in feed.setActive(value == .active) }
        .onChange(of: live, initial: true) { _, value in feed.setLive(value) }
        .onDisappear { feed.setVisible(false) }
    }

    private var fallback: String {
        if !task["interactive"].flag { return "Automatic task\nOpen this machine to follow Claude's work." }
        if live { return feed.message }
        return task["boot_log"].items.suffix(5).map(\.text).joined(separator: "\n")
    }
}
