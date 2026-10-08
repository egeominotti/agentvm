// agentvm-vm --config <vm.json>: avvia una VM e resta in vita finché non si ferma.
import Foundation

// Un terminale chiuso dall'altro capo non deve uccidere la VM: gli errori di write bastano.
signal(SIGPIPE, SIG_IGN)

let args = CommandLine.arguments
guard args.count == 3, args[1] == "--config" else {
    Events.fail("uso: agentvm-vm --config <vm.json>")
}

let runner: Runner
do {
    let config = try VMConfig.load(args[2])
    runner = Runner(configuration: try MachineFactory.make(config), ptySocket: config.ptySocket)
} catch {
    Events.fail("configurazione non valida: \(error)")
}
runner.start()
withExtendedLifetime(runner) { RunLoop.main.run() }
