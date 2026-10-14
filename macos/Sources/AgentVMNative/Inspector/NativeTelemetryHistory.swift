import Foundation

struct NativeMetricReading: Sendable {
    let at: Double
    let value: Double?
}

struct NativeMetricSeries: Identifiable, Sendable {
    let name: String
    let key: String
    let source: [NativeMetricReading]
    let samples: [NativeMetricSample]
    let latest: Double?
    let maximum: Double
    var id: String { key }

    init(name: String, key: String, points: [JSON]) {
        self.name = name
        self.key = key
        source = points.map { NativeMetricReading(at: $0["at"].number, value: $0[key].isNull ? nil : $0[key].number) }
        let measured = NativeTelemetryPolicy.sample(points: points, key: key, limit: .max)
        samples = NativeTelemetryPolicy.bounded(measured, limit: 300)
        latest = measured.last?.value
        maximum = measured.map(\.value).max() ?? 0
    }

    func nearest(at: Double) -> NativeMetricReading? {
        nearestIndex(at: at).map { source[$0] }
    }

    func step(from at: Double?, direction: Int) -> NativeMetricReading? {
        guard let current = nearestIndex(at: at ?? source.last?.at ?? 0) else { return nil }
        let next = min(source.count - 1, max(0, current + (direction < 0 ? -1 : 1)))
        return source[next]
    }

    private func nearestIndex(at: Double) -> Int? {
        guard !source.isEmpty else { return nil }
        var lower = 0
        var upper = source.count
        while lower < upper {
            let middle = lower + (upper - lower) / 2
            if source[middle].at < at { lower = middle + 1 } else { upper = middle }
        }
        if lower == 0 { return 0 }
        if lower == source.count { return source.count - 1 }
        return at - source[lower - 1].at <= source[lower].at - at ? lower - 1 : lower
    }
}

struct NativeMetricTimeline: Equatable, Sendable {
    let lower: Double
    let upper: Double
    let labels: [String]

    init(points: [JSON]) {
        lower = points.first?["at"].number ?? 0
        upper = max(lower + 1, points.last?["at"].number ?? lower)
        let includeDate = upper - lower >= 86_400
        labels = [lower, (lower + upper) / 2, upper].map {
            let date = Date(timeIntervalSince1970: $0)
            return includeDate ? date.formatted(.dateTime.month(.abbreviated).day().hour().minute())
                : date.formatted(date: .omitted, time: .standard)
        }
    }
}

struct NativeTelemetryHistory: Sendable {
    static let empty = NativeTelemetryHistory()
    let timeline: NativeMetricTimeline
    let cpu: [NativeMetricSeries]
    let memory: [NativeMetricSeries]
    let disk: [NativeMetricSeries]
    let network: [NativeMetricSeries]
    let memoryMaximum: Double
    let diskMeasured: Bool
    let isEmpty: Bool

    init(points: [JSON] = []) {
        timeline = NativeMetricTimeline(points: points)
        cpu = [NativeMetricSeries(name: "CPU", key: "cpu_pct", points: points)]
        memory = [NativeMetricSeries(name: "Used", key: "mem_used_mb", points: points),
                  NativeMetricSeries(name: "Allowed now", key: "mem_limit_mb", points: points)]
        disk = [NativeMetricSeries(name: "Read", key: "disk_read_bps", points: points),
                NativeMetricSeries(name: "Write", key: "disk_write_bps", points: points)]
        network = [NativeMetricSeries(name: "In", key: "net_rx_bps", points: points),
                   NativeMetricSeries(name: "Out", key: "net_tx_bps", points: points)]
        memoryMaximum = memory.map(\.maximum).max() ?? 0
        diskMeasured = !disk[0].samples.isEmpty
        isEmpty = points.isEmpty
    }
}
