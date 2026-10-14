enum NativePollingPolicy {
    static func overviewSeconds(active: Bool) -> Int { active ? 1 : 5 }
}
