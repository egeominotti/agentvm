import Combine
import Foundation

@MainActor
final class NativeStore: ObservableObject {
    @Published var tasks: [JSON] = []
    @Published var status: JSON = .null
    @Published var settings: JSON = .null
    @Published var limits: JSON = .null
    @Published var snapshots: [JSON] = []
    @Published var backups: [JSON] = []
    @Published var backupsError: String?
    @Published var backupsLoading = false
    @Published var storage: JSON = .null
    @Published var error: String?
    @Published var notice: String?
    @Published var connected = false
    @Published var connectionMessage: String?
    @Published var busy: Set<String> = []
    @Published var launcherPresented = false
    @Published var route: NativeRoute = .machines

    let api: NativeAPI
    private var refreshTask: Task<Void, Never>?
    private var snapshotsTask: Task<Void, Never>?
    private var snapshotsRequested = false
    private var backupsRequested = false
    private var settingsTask: Task<Void, Never>?
    private var pendingSettings: JSON?
    private var counts: [String: Int] = [:]

    init(baseURL: URL) { api = NativeAPI(baseURL: baseURL) }

    func refresh() async {
        if let refreshTask { await refreshTask.value; return }
        let task = Task { await fetchOverview() }
        refreshTask = task
        await task.value
    }

    private func fetchOverview() async {
        defer { refreshTask = nil }
        do {
            async let tasks = api.request("/api/tasks")
            async let status = api.request("/api/status")
            async let settings = api.request("/api/settings")
            let (newTasks, newStatus, view) = try await (tasks, status, settings)
            self.tasks = newTasks.items
            self.status = newStatus
            limits = view["limits"]
            // A poll must never replace a draft whose save is still pending or in flight.
            if settingsTask == nil && pendingSettings == nil { self.settings = view["settings"] }
            connected = true
            connectionMessage = nil
        } catch is CancellationError {
        } catch {
            connected = false
            connectionMessage = error.localizedDescription
        }
    }

    func refreshSnapshots(includeBackups: Bool = true) async {
        snapshotsRequested = true
        backupsRequested = backupsRequested || includeBackups
        if let snapshotsTask { await snapshotsTask.value; return }
        let task = Task { await fetchSnapshots() }
        snapshotsTask = task
        await task.value
    }

    private func fetchSnapshots() async {
        defer { snapshotsTask = nil; backupsLoading = false }
        // Coalesce overlapping refreshes, while retaining a full refresh requested by an
        // explicit backup action during a local-only poll or another cloud request.
        while snapshotsRequested {
            snapshotsRequested = false
            let includeBackups = backupsRequested
            backupsRequested = false
            await fetchSnapshotBatch(includeBackups: includeBackups)
        }
    }

    private func fetchSnapshotBatch(includeBackups: Bool) async {
        do {
            async let local = api.request("/api/snapshots")
            async let storage = api.request("/api/storage")
            let (newSnapshots, newStorage) = try await (local, storage)
            snapshots = newSnapshots.items
            self.storage = newStorage
            guard includeBackups else { return }
            backupsLoading = true
            defer { backupsLoading = false }
            do {
                backups = try await api.request("/api/backups").items
                backupsError = nil
            } catch let failure as NativeAPIError where failure.unconfiguredBackup {
                backups = []
                backupsError = nil
            } catch is CancellationError {
            } catch {
                // Backups are optional: a remote outage must leave local snapshots usable,
                // without reopening a modal every time the background refresh polls again.
                backupsError = NativeS3Presentation.message(raw: error.localizedDescription,
                                                            endpoint: settings["s3"]["endpoint"].text)
            }
        } catch is CancellationError {
        } catch {
            connected = false
            connectionMessage = error.localizedDescription
        }
    }

    @discardableResult
    func perform(_ path: String, method: String = "POST", body: JSON? = nil, success: String? = nil) async -> JSON? {
        begin(path)
        defer { end(path) }
        error = nil
        notice = nil
        do {
            let result = try await api.request(path, method: method, body: body)
            notice = success
            await refresh()
            if path.contains("snapshots") || path.contains("backups") || path.contains("storage") {
                await refreshSnapshots()
            }
            return result
        } catch is CancellationError { return nil }
        catch {
            let s3 = path.hasPrefix("/api/backups") || path.hasPrefix("/api/settings/s3") || path.hasSuffix("/backup")
            let configured = body?["config"]["endpoint"].text ?? ""
            let endpoint = configured.isEmpty ? settings["s3"]["endpoint"].text : configured
            self.error = s3 ? NativeS3Presentation.message(raw: error.localizedDescription, endpoint: endpoint)
                           : error.localizedDescription
            return nil
        }
    }

    func saveSettings(_ draft: JSON) async {
        await queueSettings(draft).value
    }

    /// Bindings enqueue in their synchronous setter, so the next edit always starts from the
    /// latest draft before a task or a network response gets another turn on the main actor.
    @discardableResult
    func queueSettings(_ draft: JSON) -> Task<Void, Never> {
        settings = draft
        pendingSettings = draft
        if let settingsTask { return settingsTask }
        let task = Task { await drainSettings() }
        settingsTask = task
        return task
    }

    private func drainSettings() async {
        let path = "/api/settings"
        begin(path)
        defer { settingsTask = nil; end(path) }
        error = nil
        notice = nil
        var settingsError: String?
        while let draft = pendingSettings {
            pendingSettings = nil
            do {
                let view = try await api.request(path, method: "PUT", body: draft)
                limits = view["limits"]
                if pendingSettings == nil {
                    settings = view["settings"]
                    if error == settingsError { error = nil }
                    notice = "Settings saved."
                }
            } catch is CancellationError { return }
            catch {
                settingsError = error.localizedDescription
                self.error = settingsError
                notice = nil
            }
        }
    }

    private func begin(_ path: String) {
        counts[path, default: 0] += 1
        busy.insert(path)
    }

    private func end(_ path: String) {
        let remaining = (counts[path] ?? 1) - 1
        if remaining == 0 { counts.removeValue(forKey: path); busy.remove(path) }
        else { counts[path] = remaining }
    }
}
