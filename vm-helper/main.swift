// agentvm-vm --config <vm.json>: avvia una VM e resta in vita finché non si ferma.
import Foundation

let args = CommandLine.arguments
guard args.count == 3, args[1] == "--config" else {
    Events.fail("uso: agentvm-vm --config <vm.json>")
}

let runner: Runner
do {
    runner = Runner(configuration: try MachineFactory.make(try VMConfig.load(args[2])))
} catch {
    Events.fail("configurazione non valida: \(error)")
}
runner.start()
withExtendedLifetime(runner) { RunLoop.main.run() }
