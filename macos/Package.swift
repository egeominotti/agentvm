// swift-tools-version: 5.9
import PackageDescription
import Foundation

var testingDependencies: [Package.Dependency] = []
var testingTargets: [Target] = []
#if compiler(>=6.0)
// Tests use the stable library matching the compiler family; production also builds on Swift 5.9.
#if compiler(>=6.4)
let testingVersion = "6.4.1"
#else
let testingVersion = "6.0.3"
#endif
testingDependencies = [.package(url: "https://github.com/swiftlang/swift-testing.git", exact: Version(stringLiteral: testingVersion))]
testingTargets = [.testTarget(name: "AgentVMNativeTests", dependencies: [
    "AgentVMNative", .product(name: "Testing", package: "swift-testing")
])]
#endif

let uiQA = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_QA_BUILD"] == "1"
let package = Package(
    name: "AgentVMNative",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "AgentVMNative", targets: ["AgentVMNative"])],
    dependencies: testingDependencies,
    targets: [
        .executableTarget(name: "AgentVMNative",
                          exclude: uiQA ? [] : ["NativeQA.swift"],
                          resources: [.copy("Resources")],
                          swiftSettings: uiQA ? [.define("NATIVE_UI_QA")] : [])
    ] + testingTargets
)
