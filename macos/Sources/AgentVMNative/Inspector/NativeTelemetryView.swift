import SwiftUI

struct NativeTelemetryView: View {
    let store: NativeStore
    @Environment(\.scenePhase) private var scenePhase
    let task: JSON
    @State private var range: String
    @State private var history = NativeTelemetryHistory.empty
    @State private var error: String?

    init(store: NativeStore, task: JSON) {
        self.store = store
        self.task = task
        _range = State(initialValue: NativeMachineLogic.ended(task) ? "all" : "5m")
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                if NativeMachineLogic.ended(task) { Text("This VM is closed. These are its recorded numbers.").font(.caption).foregroundStyle(.secondary) }
                if !NativeMachineLogic.ended(task) && !task["metrics_age_s"].isNull && task["metrics_age_s"].number > 5 {
                    Text("No new numbers for \(Int(task["metrics_age_s"].number.rounded())) seconds. These are the last recorded values.")
                        .font(.caption).foregroundStyle(.orange)
                }
                Picker("Time range", selection: $range) {
                    Text("5 min").tag("5m")
                    Text("1 hour").tag("1h")
                    Text("Whole life").tag("all")
                }.pickerStyle(.segmented)
                if let error { Text(error).font(.caption).foregroundStyle(.red) }
                NativeMetricChart(title: "CPU", series: history.cpu, timeline: history.timeline, maximum: 100) {
                    "\(Int($0.rounded()))%"
                }
                NativeMetricChart(title: "Memory", series: history.memory, timeline: history.timeline,
                    maximum: max(task["memory_mb"].number, history.memoryMaximum), format: MachineFormat.memory)
                NativeMetricChart(title: "Disk I/O", series: history.disk, timeline: history.timeline,
                    empty: history.isEmpty || history.diskMeasured ? "No data yet" : "Not measured by this VM's collector",
                    format: MachineFormat.rate)
                NativeMetricChart(title: "Network", series: history.network, timeline: history.timeline, format: MachineFormat.rate)
                if !NativeMachineLogic.ended(task) && !task["metrics"].isNull { liveDetails }
                NativeMachineFacts(task: task)
            }.padding(14)
        }
        .task(id: "\(task["id"].text)/\(range)/\(NativeMachineLogic.ended(task))/\(scenePhase == .active)") {
            guard scenePhase == .active else { return }
            repeat {
                do {
                    let points = try await store.api.request("/api/tasks/\(task["id"].text)/telemetry?range=\(range)")["points"].items
                    let prepared = await Task.detached(priority: .utility) { NativeTelemetryHistory(points: points) }.value
                    guard !Task.isCancelled else { return }
                    history = prepared
                    error = nil
                } catch is CancellationError { }
                catch { if !Task.isCancelled { self.error = error.localizedDescription } }
                if NativeMachineLogic.ended(task) { break }
                try? await Task.sleep(for: .seconds(range == "5m" ? 2 : 15))
            } while !Task.isCancelled
        }
    }

    private var liveDetails: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Live machine").font(.headline)
            Text("Uptime \(MachineFormat.duration(task["metrics"]["uptime_s"].number)) · \(task["metrics"]["procs"].integer) processes · load \(task["metrics"]["load1"].number, specifier: "%.2f")")
                .font(.caption).foregroundStyle(.secondary)
            Text("Disk \(MachineFormat.memory(task["metrics"]["disk_used_mb"].number)) / \(MachineFormat.memory(task["metrics"]["disk_total_mb"].number))")
                .font(.caption)
            ProgressView(value: task["metrics"]["disk_used_mb"].number, total: max(1, task["metrics"]["disk_total_mb"].number))
            processList("Busiest (CPU)", rows: task["metrics"]["top"].items, cpu: true)
            processList("Largest (memory)", rows: task["metrics"]["top_mem"].items, cpu: false)
            if !task["metrics"]["ports"].items.isEmpty {
                Text("Listening ports").font(.headline)
                ForEach(task["metrics"]["ports"].items, id: \.["port"].integer) { port in
                    Text("\(port["name"].text) · :\(port["port"].integer)").font(.caption.monospaced())
                }
            }
        }
    }

    private func processList(_ title: String, rows: [JSON], cpu: Bool) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.headline)
            if rows.isEmpty { Text("No process data").font(.caption).foregroundStyle(.secondary) }
            ForEach(Array(rows.enumerated()), id: \.offset) { _, process in
                HStack {
                    Text(process["name"].text).lineLimit(1)
                    Spacer()
                    if cpu { Text("\(Int(process["cpu_pct"].number.rounded()))%") }
                    Text(MachineFormat.memory(process["mem_mb"].number))
                }.font(.caption)
            }
        }
    }
}

struct NativeMachineFacts: View {
    let task: JSON
    private var facts: [(String, String)] {
        [
            ("State", NativeMachineLogic.status(task)), ("Model", NativeAgentPresentation.model(task)),
            ("Agent", NativeAgentPresentation.name(task)), ("CLI version", NativeAgentPresentation.version(task)),
            ("Resources", "\(task["cpus"].integer) vCPUs, \(MachineFormat.memory(task["memory_mb"].number))"),
            ("Repository", task["repo"].text), ("Branch", task["branch"].text),
            ("From commit", String(task["base_sha"].text.prefix(10))), ("Age", MachineFormat.age(task)), ("Id", task["id"].text)
        ]
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("This machine").font(.headline)
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
                ForEach(facts, id: \.0) { fact in
                    GridRow { Text(fact.0).foregroundStyle(.secondary); Text(fact.1).frame(maxWidth: .infinity, alignment: .leading) }
                }
            }.font(.caption).textSelection(.enabled)
        }
    }
}
