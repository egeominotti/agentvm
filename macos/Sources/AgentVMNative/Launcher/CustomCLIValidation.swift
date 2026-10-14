import Foundation

enum NativeCustomCLIValidation {
    static func blocker(_ custom: JSON, interactive: Bool) -> String? {
        let name = custom["name"].text
        if name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || name.utf8.count > 80 || hasControl(name) {
            return "Give the custom CLI a name of 1–80 characters."
        }
        if !interactive && custom["headless"].text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return "Add a headless command for automatic tasks."
        }
        for key in ["interactive", "headless", "resume"] {
            let command = custom[key].text
            if key != "interactive" && custom[key].isNull { continue }
            if command.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || command.utf8.count > 8192 || hasControl(command) {
                return "Custom CLI commands must be nonempty single lines."
            }
            let executable = command.trimmingCharacters(in: .whitespaces).drop(while: { $0 == "'" || $0 == "\"" }).split(whereSeparator: \.isWhitespace).first ?? ""
            if executable.hasPrefix("-") || executable.contains("{") { return "Use a fixed executable name for the custom CLI." }
            var quote: Character?
            var escaped = false
            for character in command {
                if escaped { escaped = false; continue }
                if character == "\\" && quote != "'" { escaped = true; continue }
                if let current = quote { if character == current { quote = nil } }
                else if character == "'" || character == "\"" { quote = character }
                else if ";|&<>`$".contains(character) { return "Launch commands accept arguments and placeholders, without shell expressions." }
            }
            if quote != nil || escaped { return "Finish the quote or escape in the custom CLI command." }
        }
        return nil
    }

    private static func hasControl(_ text: String) -> Bool {
        text.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
    }
}
