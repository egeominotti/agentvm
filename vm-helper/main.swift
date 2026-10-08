// agentvm-vm --config <vm.json>: starts a VM and stays alive until it stops.
import Foundation

// A terminal closed by the other end must not kill the VM: write errors are enough.
signal(SIGPIPE, SIG_IGN)

let args = CommandLine.arguments
guard args.count == 3, args[1] == "--config" else {
    Events.fail("usage: agentvm-vm --config <vm.json>")
}

let runner: Runner
do {
    let config = try VMConfig.load(args[2])
    runner = Runner(configuration: try MachineFactory.make(config), ptySocket: config.ptySocket)
} catch {
    Events.fail("invalid configuration: \(error)")
}
runner.start()
withExtendedLifetime(runner) { RunLoop.main.run() }
