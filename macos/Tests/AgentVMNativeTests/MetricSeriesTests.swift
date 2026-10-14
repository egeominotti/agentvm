import Testing
@testable import AgentVMNative

@Suite("Native cached telemetry details")
struct MetricSeriesTests {
    @Test("Missing tooltip readings remain missing while latest and peak use measured values")
    func missingTooltip() {
        let points: [JSON] = [.object(["at": JSON(10), "cpu_pct": JSON(9)]),
                              .object(["at": JSON(20), "cpu_pct": .null]),
                              .object(["at": JSON(30), "cpu_pct": JSON(7)]),
                              .object(["at": JSON(40), "cpu_pct": .null])]
        let series = NativeMetricSeries(name: "CPU", key: "cpu_pct", points: points)
        #expect(series.latest == 7)
        #expect(series.maximum == 9)
        #expect(series.source.count == 4)
        #expect(series.nearest(at: 20)?.at == 20)
        #expect(series.nearest(at: 20)?.value == nil)
        #expect(series.nearest(at: 21)?.value == nil)
        #expect(series.nearest(at: 25)?.at == 20)
        #expect(series.nearest(at: 0)?.value == 9)
        #expect(series.nearest(at: 100)?.at == 40)
        #expect(series.nearest(at: 100)?.value == nil)
    }

    @Test("Rendering is bounded while selection still finds exact full-history readings")
    func exactFullHistorySelection() {
        let points = (0..<5000).map { JSON.object(["at": JSON($0), "mem_used_mb": JSON($0)]) }
        let series = NativeMetricSeries(name: "Memory", key: "mem_used_mb", points: points)
        #expect(series.samples.count <= 300)
        #expect(series.source.count == 5000)
        #expect(series.nearest(at: 2001)?.at == 2001)
        #expect(series.nearest(at: 2001)?.value == 2001)
        #expect(series.latest == 4999)
        #expect(series.maximum == 4999)
    }

    @Test("Empty and unmeasured histories do not invent values")
    func noMeasurements() {
        let empty = NativeMetricSeries(name: "CPU", key: "cpu_pct", points: [])
        #expect(empty.latest == nil)
        #expect(empty.nearest(at: 10) == nil)
        #expect(empty.samples.isEmpty)
        let missing = NativeMetricSeries(name: "Disk", key: "disk_read_bps", points: [.object(["at": JSON(5)])])
        #expect(missing.latest == nil)
        #expect(missing.samples.isEmpty)
        #expect(missing.nearest(at: 5)?.at == 5)
        #expect(missing.nearest(at: 5)?.value == nil)
    }

    @Test("A single timestamp retains a finite chart extent")
    func timelineExtent() {
        let timeline = NativeMetricTimeline(points: [.object(["at": JSON(100)])])
        #expect(timeline.lower == 100)
        #expect(timeline.upper == 101)
        #expect(timeline.labels.count == 3)
    }
}
