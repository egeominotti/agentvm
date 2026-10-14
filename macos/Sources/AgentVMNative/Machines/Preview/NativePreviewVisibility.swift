import AppKit
import SwiftUI

struct NativePreviewVisibility: NSViewRepresentable {
    let changed: (Bool) -> Void
    func makeNSView(context: Context) -> NativePreviewVisibilityView { NativePreviewVisibilityView(changed: changed) }
    func updateNSView(_ view: NativePreviewVisibilityView, context: Context) { view.refresh() }
    static func dismantleNSView(_ view: NativePreviewVisibilityView, coordinator: ()) { view.close() }
}

@MainActor
final class NativePreviewVisibilityView: NSView {
    private let changed: (Bool) -> Void
    private var observers: [NSObjectProtocol] = []
    private weak var clip: NSClipView?
    private var pending = false
    private var closed = false

    init(changed: @escaping (Bool) -> Void) { self.changed = changed; super.init(frame: .zero) }
    required init?(coder: NSCoder) { fatalError("Use init(changed:)") }
    override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); observe(); refresh() }
    override func viewDidMoveToSuperview() { super.viewDidMoveToSuperview(); observe(); refresh() }
    override func layout() { super.layout(); if clip !== enclosingScrollView?.contentView { observe() }; refresh() }

    func refresh() {
        guard !closed, !pending else { return }
        pending = true
        // Wait for AppKit to settle a reflow before deciding whether to detach its stream.
        DispatchQueue.main.async { [weak self] in
            guard let self, !closed else { return }
            pending = false
            let shown = window?.isVisible == true && window?.isMiniaturized == false
                && !isHiddenOrHasHiddenAncestor && !visibleRect.intersection(bounds).isEmpty
            changed(shown)
        }
    }

    private func observe() {
        guard !closed else { return }
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        clip = enclosingScrollView?.contentView
        if let clip {
            clip.postsBoundsChangedNotifications = true
            watch(NSView.boundsDidChangeNotification, object: clip)
        }
        postsFrameChangedNotifications = true
        watch(NSView.frameDidChangeNotification, object: self)
        if let window {
            for name in [NSWindow.didResizeNotification, NSWindow.didMoveNotification, NSWindow.didMiniaturizeNotification,
                         NSWindow.didDeminiaturizeNotification, NSWindow.didChangeOcclusionStateNotification] { watch(name, object: window) }
        }
    }

    private func watch(_ name: Notification.Name, object: AnyObject) {
        observers.append(NotificationCenter.default.addObserver(forName: name, object: object, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        })
    }

    func close() {
        closed = true
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        changed(false)
    }
}
