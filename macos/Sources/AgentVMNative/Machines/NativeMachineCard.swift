import SwiftUI

struct NativeMachineCard: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @Binding var selected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                NativeMachineStatus(task: task)
                if !NativeAgentPresentation.showsClaudeAnalytics(task) {
                    Text(NativeAgentPresentation.name(task)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                if NativeMachineLogic.ended(task) {
                    Toggle("Select", isOn: $selected).labelsHidden().toggleStyle(.checkbox)
                        .accessibilityLabel("Select \(NativeMachineLogic.title(task))")
                        .accessibilityIdentifier("machine.select.\(task["id"].text)")
                }
            }
            Button { store.route = .machine(task["id"].text) } label: {
                VStack(alignment: .leading, spacing: 4) {
                    Text(NativeMachineLogic.title(task)).font(.headline).lineLimit(2)
                    Text("\(URL(fileURLWithPath: task["repo"].text).lastPathComponent) · #\(task["id"].text.suffix(4))")
                        .font(.caption).foregroundStyle(.secondary)
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.buttonStyle(.plain)
            NativeMachinePreview(store: store, task: task).frame(height: 112)
            NativeMachineNumbers(task: task)
            HStack {
                Text(MachineFormat.age(task)).font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Open") { store.route = .machine(task["id"].text) }
                    .accessibilityLabel("Open \(NativeMachineLogic.title(task))")
                    .accessibilityIdentifier("machine.open.\(task["id"].text)")
            }
        }
        .padding(15)
        .background(.background, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(.quaternary))
    }
}

struct NativeMachineRow: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @Binding var selected: Bool

    var body: some View {
        ViewThatFits(in: .horizontal) {
            wideRow.frame(minWidth: 1000)
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    selectionToggle
                    title
                    Spacer(minLength: 8)
                    openButton.fixedSize()
                }
                HStack {
                    NativeMachineStatus(task: task)
                    Spacer(minLength: 8)
                    NativeMachineNumbers(task: task)
                    age
                }
                Text(task["branch"].text).font(.caption.monospaced()).foregroundStyle(.secondary)
                    .lineLimit(1).help(task["branch"].text)
            }
        }.padding(12).background(Color.secondary.opacity(0.07), in: RoundedRectangle(cornerRadius: 8))
    }

    private var wideRow: some View {
        HStack(spacing: 14) {
            selectionToggle
            NativeMachineStatus(task: task).frame(width: 145, alignment: .leading)
            title
            Text(task["branch"].text).font(.caption.monospaced()).lineLimit(1).frame(maxWidth: 170)
            NativeMachineNumbers(task: task).frame(width: 190)
            age.frame(width: 75)
            openButton.fixedSize()
        }
    }

    @ViewBuilder private var selectionToggle: some View {
        if NativeMachineLogic.ended(task) {
            Toggle("Select", isOn: $selected).labelsHidden().toggleStyle(.checkbox)
                .accessibilityLabel("Select \(NativeMachineLogic.title(task))")
                .accessibilityIdentifier("machine.select.\(task["id"].text)")
        }
    }

    private var title: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(NativeMachineLogic.title(task)).font(.headline).lineLimit(1)
            Text(NativeAgentPresentation.showsClaudeAnalytics(task) ? task["repo"].text
                 : "\(NativeAgentPresentation.name(task)) · \(task["repo"].text)")
                .font(.caption).foregroundStyle(.secondary).lineLimit(1).help(task["repo"].text)
        }.frame(maxWidth: .infinity, alignment: .leading).layoutPriority(1)
    }

    private var age: some View { Text(MachineFormat.age(task)).font(.caption).foregroundStyle(.secondary) }

    private var openButton: some View {
        Button("Open") { store.route = .machine(task["id"].text) }
            .accessibilityLabel("Open \(NativeMachineLogic.title(task))")
            .accessibilityIdentifier("machine.open.\(task["id"].text)")
    }
}

struct NativeMachineStatus: View {
    let task: JSON
    private var color: Color {
        switch NativeMachineLogic.state(task) {
        case "failed": return .red
        case "done", "no_changes": return .green
        case "stopped": return .secondary
        default: return NativeMachineLogic.category(task) == "waiting" ? .orange : .blue
        }
    }
    var body: some View {
        Label(NativeMachineLogic.status(task), systemImage: "circle.fill")
            .font(.caption).foregroundStyle(color)
    }
}

struct NativeMachineNumbers: View {
    let task: JSON
    var body: some View {
        HStack(spacing: 12) {
            if !task["metrics"].isNull {
                Label("\(Int(task["metrics"]["cpu_pct"].number.rounded()))%", systemImage: "cpu")
                Label(MachineFormat.memory(task["metrics"]["mem_used_mb"].number), systemImage: "memorychip")
            }
            if NativeAgentPresentation.showsClaudeAnalytics(task), !task["usage"].isNull {
                Text(MachineFormat.money(task["usage"]["cost_usd"].number))
            }
        }.font(.caption).foregroundStyle(.secondary)
    }
}
