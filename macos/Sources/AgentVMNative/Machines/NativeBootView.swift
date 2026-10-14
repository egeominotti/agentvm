import SwiftUI

struct NativeBootView: View {
    @ObservedObject var store: NativeStore
    let task: JSON

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                ProgressView().controlSize(.large)
                VStack(alignment: .leading) {
                    Text(NativeMachineLogic.state(task) == "collecting" ? "Saving the work and shutting down…" : NativeMachineLogic.status(task))
                        .font(.title2.bold())
                    Text(MachineFormat.age(task)).foregroundStyle(.secondary)
                }
            }
            if NativeMachineLogic.state(task) == "queued" {
                Text("Up to \(store.status["concurrency"].integer) VMs run at a time. This one starts when a slot and enough memory are free.")
                Button("Open Resources in Settings") {
                    UserDefaults.standard.set("resources", forKey: "native.settingsSection")
                    store.route = .settings
                }
            } else if NativeMachineLogic.state(task) == "collecting" {
                Text("The commits go to \(task["branch"].text) in your repository.")
            }
            if NativeMachineLogic.state(task) != "collecting" {
                let steps = NativeBootLogic.steps(task)
                ProgressView(value: Double(steps.prefix(while: \.done).count), total: Double(steps.count))
                ForEach(steps) { step in
                    HStack {
                        Image(systemName: step.failed ? "exclamationmark.circle.fill" : step.done ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(step.failed ? Color.orange : step.done ? .green : .secondary)
                        Text(step.label)
                        Spacer()
                        Text(step.detail).foregroundStyle(.secondary)
                    }.font(.caption)
                }
            }
            if !task["boot_log"].items.isEmpty {
                ScrollView {
                    Text(task["boot_log"].items.map(\.text).joined(separator: "\n"))
                        .font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }.frame(maxHeight: 220)
            }
        }.padding(28).frame(maxWidth: 550).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
    }
}
