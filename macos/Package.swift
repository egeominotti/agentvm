// swift-tools-version: 5.9
import PackageDescription
import Foundation

var testingTargets: [Target] = []
#if compiler(>=6.0)
// Swift 6 includes a matching Testing library; its source package is not a production dependency.
testingTargets = [.testTarget(name: "AgentVMNativeTests", dependencies: ["AgentVMNative"])]
#endif

let uiQA = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_QA_BUILD"] == "1"
let package = Package(
    name: "AgentVMNative",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "AgentVMNative", targets: ["AgentVMNative"])],
    targets: [
        .executableTarget(name: "AgentVMNative",
                          exclude: uiQA ? [] : ["NativeQA.swift"],
                          resources: [.copy("Resources")],
                          swiftSettings: uiQA ? [.define("NATIVE_UI_QA")] : [])
    ] + testingTargets
)
