import SwiftUI

struct NativeMetricChart: View {
    let title: String
    let series: [NativeMetricSeries]
    let timeline: NativeMetricTimeline
    var maximum: Double? = nil
    var empty = "No data yet"
    let format: (Double) -> String
    @State private var selected: Double?
    @State private var selectedText = ""
    @FocusState private var focused: Bool

    private var ceiling: Double { max(1, maximum ?? series.map(\.maximum).max() ?? 1) }
    private var colors: [Color] { [.accentColor, .orange] }
    private var latest: String {
        series.compactMap { line in
            guard let value = line.latest else { return nil }
            return series.count == 1 ? format(value) : "\(line.name) \(format(value))"
        }.joined(separator: " · ")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack { Text(title).font(.headline); Spacer(); Text(latest).font(.caption).foregroundStyle(.secondary) }
            if series.allSatisfy({ $0.samples.isEmpty }) {
                Text(empty).font(.caption).foregroundStyle(.secondary).frame(maxWidth: .infinity, minHeight: 90)
            } else {
                HStack(alignment: .top, spacing: 8) {
                    VStack {
                        Text(format(ceiling))
                        Spacer()
                        Text(format(ceiling / 2))
                        Spacer()
                        Text(format(0))
                    }.font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                        .frame(width: 58, height: 118, alignment: .trailing)
                    VStack(spacing: 4) {
                        plot.frame(height: 120)
                        ViewThatFits(in: .horizontal) {
                            HStack(spacing: 8) {
                                Text(timeline.labels[0]).fixedSize()
                                Spacer(minLength: 8)
                                Text(timeline.labels[1]).fixedSize()
                                Spacer(minLength: 8)
                                Text(timeline.labels[2]).fixedSize()
                            }
                            HStack(spacing: 8) {
                                Text(timeline.labels[0]).fixedSize()
                                Spacer(minLength: 8)
                                Text(timeline.labels[2]).fixedSize()
                            }
                            Text(timeline.labels[1]).fixedSize(horizontal: true, vertical: false)
                        }.font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity).lineLimit(1)
                    }
                }
                if series.count > 1 {
                    HStack(spacing: 16) {
                        ForEach(Array(series.enumerated()), id: \.element.id) { index, line in
                            Label { Text(line.name) } icon: { Circle().fill(colors[index % colors.count]).frame(width: 7, height: 7) }
                        }
                    }.font(.caption).foregroundStyle(.secondary)
                }
                if !selectedText.isEmpty { Text(selectedText).font(.caption).foregroundStyle(.secondary) }
            }
        }.padding(12).background(Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 10))
            .transaction { $0.animation = nil }
            .onChange(of: timeline) { _, _ in if let selected { select(selected) } }
    }

    private var plot: some View {
        GeometryReader { geometry in
            Canvas(rendersAsynchronously: true) { context, size in
                var grid = Path()
                for fraction in [0.0, 0.5, 1.0] {
                    let y = size.height * fraction
                    grid.move(to: CGPoint(x: 0, y: y)); grid.addLine(to: CGPoint(x: size.width, y: y))
                }
                context.stroke(grid, with: .color(.secondary.opacity(0.2)), style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
                for (index, line) in series.enumerated() {
                    var path = Path()
                    var segment: Int?
                    for (offset, point) in line.samples.enumerated() {
                        let position = CGPoint(x: (point.at - timeline.lower) / (timeline.upper - timeline.lower) * size.width,
                                               y: size.height * (1 - min(1, max(0, point.value / ceiling))))
                        if segment == point.segment { path.addLine(to: position) }
                        else { path.move(to: position); segment = point.segment }
                        let first = offset == 0 || line.samples[offset - 1].segment != point.segment
                        let last = offset == line.samples.count - 1 || line.samples[offset + 1].segment != point.segment
                        if first && last {
                            context.fill(Path(ellipseIn: CGRect(x: position.x - 2, y: position.y - 2, width: 4, height: 4)),
                                         with: .color(colors[index % colors.count]))
                        }
                    }
                    context.stroke(path, with: .color(colors[index % colors.count]), lineWidth: 1.5)
                }
                if let selected {
                    let x = (selected - timeline.lower) / (timeline.upper - timeline.lower) * size.width
                    var rule = Path()
                    rule.move(to: CGPoint(x: x, y: 0)); rule.addLine(to: CGPoint(x: x, y: size.height))
                    context.stroke(rule, with: .color(.secondary), lineWidth: 1)
                }
            }.clipped().contentShape(Rectangle())
                .focusable()
                .focused($focused)
                .onKeyPress(.leftArrow) { step(-1); return .handled }
                .onKeyPress(.rightArrow) { step(1); return .handled }
                .gesture(DragGesture(minimumDistance: 0).onChanged { event in
                    focused = true
                    let fraction = min(1, max(0, event.location.x / max(1, geometry.size.width)))
                    select(timeline.lower + fraction * (timeline.upper - timeline.lower))
                })
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("\(title) history")
                .accessibilityValue(selectedText.isEmpty ? latest : selectedText)
                .accessibilityAdjustableAction { direction in
                    switch direction {
                    case .increment: step(1)
                    case .decrement: step(-1)
                    @unknown default: break
                    }
                }
        }
    }

    private func step(_ direction: Int) {
        if let reading = series.first?.step(from: selected, direction: direction) { select(reading.at) }
    }

    private func select(_ at: Double) {
        let at = min(timeline.upper, max(timeline.lower, at))
        selected = at
        let values = series.map { line in
            let value = line.nearest(at: at)?.value.map(format) ?? "not measured"
            return "\(line.name) \(value)"
        }.joined(separator: " · ")
        let time = series.first?.nearest(at: at)?.at ?? at
        selectedText = "\(Date(timeIntervalSince1970: time).formatted(date: .omitted, time: .standard)): \(values)"
    }
}
