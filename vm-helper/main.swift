// agentvm-vm --config <vm.json>: starts a VM and stays alive until it stops.
import Foundation

// A terminal closed by the other end must not kill the VM: write errors are enough.
signal(SIGPIPE, SIG_IGN)

// Every terminal, preview and forwarded connection uses two descriptors: lift the default 256.
var files = rlimit()
if getrlimit(RLIMIT_NOFILE, &files) == 0 {
    files.rlim_cur = min(files.rlim_max, 8192)
    setrlimit(RLIMIT_NOFILE, &files)
}

let args = CommandLine.arguments
guard args.count == 3, args[1] == "--config" else {
    Events.fail("usage: agentvm-vm --config <vm.json>")
}

let runner: Runner
do {
    let config = try VMConfig.load(args[2])
    runner = Runner(configuration: try MachineFactory.make(config), ptySocket: config.ptySocket, balloon: config.balloon,
                    memoryMB: config.memoryMB)
} catch {
    Events.fail("invalid configuration: \(error)")
}
runner.start()
withExtendedLifetime(runner) { RunLoop.main.run() }
