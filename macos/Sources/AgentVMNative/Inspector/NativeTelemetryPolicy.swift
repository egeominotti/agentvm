import Foundation

struct NativeMetricSample: Identifiable, Sendable {
    let index: Int
    let at: Double
    let value: Double
    let segment: Int
    var id: Int { index }
}

enum NativeTelemetryPolicy {
    static func sample(points: [JSON], key: String, limit: Int = 300) -> [NativeMetricSample] {
        var result: [NativeMetricSample] = []
        var segment = 0
        for (index, point) in points.enumerated() {
            if point[key].isNull { segment += 1; continue }
            result.append(NativeMetricSample(index: index, at: point["at"].number,
                                             value: point[key].number, segment: segment))
        }
        return bounded(result, limit: limit)
    }

    static func bounded(_ source: [NativeMetricSample], limit: Int) -> [NativeMetricSample] {
        guard limit > 0 else { return [] }
        guard source.count > limit else { return source }
        guard limit >= 4 else { return minMax(source, limit: limit) }
        var anchors: [Int: NativeMetricSample] = [:]
        for (offset, point) in source.enumerated() {
            let first = offset == 0 || source[offset - 1].segment != point.segment
            let last = offset == source.count - 1 || source[offset + 1].segment != point.segment
            if first || last { anchors[point.index] = point }
        }
        for point in [source.min(by: { $0.value < $1.value }), source.max(by: { $0.value < $1.value })].compactMap({ $0 }) {
            anchors[point.index] = point
        }
        // Keep short measured intervals visible before allocating space to long trends.
        // More interval endpoints than pixels require representative rendering; selection
        // still uses every original reading in NativeMetricSeries.
        guard anchors.count <= limit else { return minMax(source, limit: limit) }
        for point in minMax(source, limit: limit - anchors.count) { anchors[point.index] = point }
        return anchors.values.sorted { $0.index < $1.index }
    }

    private static func minMax(_ source: [NativeMetricSample], limit: Int) -> [NativeMetricSample] {
        guard limit > 0 else { return [] }
        guard source.count > limit else { return source }
        guard limit > 1 else { return Array(source.suffix(1)) }
        guard limit > 2 else { return [source[0], source[source.count - 1]] }
        let buckets = max(1, (limit - 2) / 2)
        let middle = source.count - 2
        var result = [source[0]]
        // Keep each bucket's low and high values instead of averaging away short spikes.
        for bucket in 0..<buckets {
            let start = 1 + bucket * middle / buckets
            let end = 1 + (bucket + 1) * middle / buckets
            let values = source[start..<end]
            guard let low = values.min(by: { $0.value < $1.value }),
                  let high = values.max(by: { $0.value < $1.value }) else { continue }
            if limit == 3 { result.append(high); break }
            result.append(low.index < high.index ? low : high)
            if low.index != high.index { result.append(low.index < high.index ? high : low) }
        }
        result.append(source[source.count - 1])
        return result
    }
}
