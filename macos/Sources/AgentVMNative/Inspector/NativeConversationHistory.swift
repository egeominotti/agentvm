struct NativeConversationHistory: Sendable {
    static let empty = NativeConversationHistory()
    let usageTimeline: NativeMetricTimeline
    let outputTimeline: NativeMetricTimeline
    let cost: [NativeMetricSeries]
    let output: [NativeMetricSeries]
    let context: [NativeMetricSeries]

    init(entries: [JSON] = [], samples: [JSON] = []) {
        let turns = NativeInspectorLogic.turns(entries)
        usageTimeline = NativeMetricTimeline(points: samples)
        outputTimeline = NativeMetricTimeline(points: turns)
        cost = [NativeMetricSeries(name: "Cost", key: "cost_usd", points: samples)]
        output = [NativeMetricSeries(name: "Output", key: "output_tokens", points: turns)]
        context = [NativeMetricSeries(name: "Context", key: "context_pct", points: samples)]
    }
}
