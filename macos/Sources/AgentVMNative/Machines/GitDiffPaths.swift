import Foundation

enum NativeGitDiffPaths {
    static func metadata(_ line: String) -> String? {
        for prefix in ["--- ", "+++ "] where line.hasPrefix(prefix) {
            let path = decode(metadataField(String(line.dropFirst(prefix.count))))
            guard path != "/dev/null" else { return nil }
            return stripSide(path)
        }
        if line.hasPrefix("rename to ") { return decode(String(line.dropFirst(10))) }
        return nil
    }

    static func header(_ line: String) -> String? {
        let body = String(line.dropFirst("diff --git ".count))
        if body.hasPrefix("\"") {
            let bytes = Array(body.utf8)
            var escaped = false
            for index in 1..<bytes.count {
                if escaped { escaped = false; continue }
                if bytes[index] == 92 { escaped = true; continue }
                if bytes[index] == 34 {
                    let rest = String(decoding: bytes[(index + 1)...], as: UTF8.self).trimmingCharacters(in: .whitespaces)
                    return stripSide(decode(rest))
                }
            }
            return nil
        }
        if let quoted = body.range(of: " \"b/") { return stripSide(decode(String(body[quoted.lowerBound...].dropFirst()))) }
        var start = body.startIndex
        var fallback: String?
        while let delimiter = body.range(of: " b/", range: start..<body.endIndex) {
            let destination = String(body[delimiter.upperBound...])
            // Git leaves spaces unquoted. Matching both sides disambiguates a filename
            // containing the same separator that divides the header's source and target.
            if stripSide(String(body[..<delimiter.lowerBound])) == destination { return destination }
            fallback = destination
            start = delimiter.upperBound
        }
        return fallback
    }

    private static func stripSide(_ path: String) -> String {
        path.hasPrefix("a/") || path.hasPrefix("b/") ? String(path.dropFirst(2)) : path
    }

    private static func metadataField(_ raw: String) -> String {
        guard raw.hasPrefix("\"") else { return raw.components(separatedBy: "\t").first ?? raw }
        var escaped = false
        for index in raw.indices.dropFirst() {
            if escaped { escaped = false; continue }
            if raw[index] == "\\" { escaped = true; continue }
            if raw[index] == "\"" { return String(raw[...index]) }
        }
        return raw
    }

    private static func decode(_ path: String) -> String {
        guard path.hasPrefix("\""), path.hasSuffix("\"") else { return path }
        let input = Array(path.utf8.dropFirst().dropLast())
        var output: [UInt8] = []
        var index = 0
        let escapes: [UInt8: UInt8] = [97: 7, 98: 8, 102: 12, 110: 10, 114: 13, 116: 9, 118: 11]
        while index < input.count {
            let byte = input[index]
            index += 1
            guard byte == 92, index < input.count else { output.append(byte); continue }
            var escaped = input[index]
            index += 1
            if (48...55).contains(escaped) {
                var value = Int(escaped - 48)
                var digits = 1
                while digits < 3, index < input.count, (48...55).contains(input[index]) {
                    value = value * 8 + Int(input[index] - 48)
                    index += 1
                    digits += 1
                }
                escaped = UInt8(truncatingIfNeeded: value)
            } else { escaped = escapes[escaped] ?? escaped }
            output.append(escaped)
        }
        return String(decoding: output, as: UTF8.self)
    }
}
