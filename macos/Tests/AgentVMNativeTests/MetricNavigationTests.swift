import Testing
@testable import AgentVMNative

@Suite("Full-resolution chart navigation")
struct MetricNavigationTests {
    @Test("Arrow navigation visits every original reading including missing measurements")
    func everyReading() {
        let points = (0..<5000).map { JSON.object(["at": JSON($0), "cpu_pct": $0 == 2002 ? .null : JSON($0)]) }
        let series = NativeMetricSeries(name: "CPU", key: "cpu_pct", points: points)
        #expect(series.step(from: 2001, direction: 1)?.at == 2002)
        #expect(series.step(from: 2001, direction: 1)?.value == nil)
        #expect(series.step(from: 2002, direction: -1)?.at == 2001)
        #expect(series.step(from: nil, direction: -1)?.at == 4998)
    }

    @Test("Navigation clamps to available timestamps instead of leaving the history")
    func bounds() {
        let series = NativeMetricSeries(name: "CPU", key: "cpu_pct", points: [
            .object(["at": JSON(10), "cpu_pct": JSON(1)]), .object(["at": JSON(100), "cpu_pct": JSON(2)])
        ])
        #expect(series.step(from: 10, direction: -1)?.at == 10)
        #expect(series.step(from: 100, direction: 1)?.at == 100)
        #expect(series.step(from: nil, direction: 1)?.at == 100)
        #expect(series.step(from: 20, direction: 1)?.at == 100)
        #expect(NativeMetricSeries(name: "Empty", key: "cpu_pct", points: []).step(from: nil, direction: 1) == nil)
    }
}
