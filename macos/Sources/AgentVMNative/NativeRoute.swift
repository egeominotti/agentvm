import Foundation

enum NativeRoute: Hashable {
    case machines
    case snapshots
    case settings
    case machine(String)
}
