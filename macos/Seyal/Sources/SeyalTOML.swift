import Foundation

enum SeyalTOMLValue: Equatable, Sendable {
    case string(String)
    case bool(Bool)
    case number(Double)
    case array([SeyalTOMLValue])
    case table([String: SeyalTOMLValue])

    var string: String? {
        if case let .string(value) = self { return value }
        return nil
    }

    var bool: Bool? {
        if case let .bool(value) = self { return value }
        return nil
    }

    var number: Double? {
        if case let .number(value) = self { return value }
        return nil
    }

    var stringArray: [String]? {
        guard case let .array(values) = self else { return nil }
        var strings: [String] = []
        strings.reserveCapacity(values.count)
        for value in values {
            guard let string = value.string else { return nil }
            strings.append(string)
        }
        return strings
    }

    var table: [String: SeyalTOMLValue]? {
        if case let .table(value) = self { return value }
        return nil
    }

    subscript(key: String) -> SeyalTOMLValue? {
        table?[key]
    }
}

enum SeyalTOMLError: Error, Equatable, CustomStringConvertible {
    case message(String)

    var description: String {
        switch self {
        case let .message(message): message
        }
    }
}

enum SeyalTOMLParser {
    static func parse(_ text: String) -> Result<[String: SeyalTOMLValue], SeyalTOMLError> {
        var root: [String: SeyalTOMLValue] = [:]
        var currentPath: [String] = []
        let lines = text.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline)
        for (index, rawLine) in lines.enumerated() {
            let lineNumber = index + 1
            let stripped = stripComment(String(rawLine)).trimmingCharacters(in: .whitespaces)
            if stripped.isEmpty { continue }
            if stripped.hasPrefix("[") {
                guard stripped.hasSuffix("]") else {
                    return .failure(.message("line \(lineNumber): invalid table header"))
                }
                let name = String(stripped.dropFirst().dropLast()).trimmingCharacters(in: .whitespaces)
                guard !name.isEmpty else {
                    return .failure(.message("line \(lineNumber): empty table name"))
                }
                currentPath = name.split(separator: ".").map(String.init)
                continue
            }
            guard let equals = stripped.firstIndex(of: "=") else {
                return .failure(.message("line \(lineNumber): expected key = value"))
            }
            let key = stripped[..<equals].trimmingCharacters(in: .whitespaces)
            let rawValue = stripped[stripped.index(after: equals)...].trimmingCharacters(in: .whitespaces)
            guard !key.isEmpty else {
                return .failure(.message("line \(lineNumber): empty key"))
            }
            switch parseValue(String(rawValue)) {
            case let .success(value):
                set(root: &root, path: currentPath + [key], value: value)
            case let .failure(error):
                return .failure(.message("line \(lineNumber): \(error.description)"))
            }
        }
        return .success(root)
    }

    private static func set(
        root: inout [String: SeyalTOMLValue],
        path: [String],
        value: SeyalTOMLValue
    ) {
        func write(_ remaining: [String], into table: inout [String: SeyalTOMLValue]) {
            guard let head = remaining.first else { return }
            let rest = Array(remaining.dropFirst())
            if rest.isEmpty {
                table[head] = value
                return
            }
            var child = table[head]?.table ?? [:]
            write(rest, into: &child)
            table[head] = .table(child)
        }
        write(path, into: &root)
    }

    private static func stripComment(_ line: String) -> String {
        var inString = false
        var escaped = false
        for (index, character) in line.enumerated() {
            if escaped {
                escaped = false
                continue
            }
            if character == "\\" && inString {
                escaped = true
                continue
            }
            if character == "\"" {
                inString.toggle()
                continue
            }
            if character == "#" && !inString {
                return String(line.prefix(index))
            }
        }
        return line
    }

    private static func parseValue(_ raw: String) -> Result<SeyalTOMLValue, SeyalTOMLError> {
        if raw == "true" { return .success(.bool(true)) }
        if raw == "false" { return .success(.bool(false)) }
        if raw.hasPrefix("\"") {
            return parseString(raw).map(SeyalTOMLValue.string)
        }
        if raw.hasPrefix("[") {
            return parseArray(raw)
        }
        if let number = Double(raw) {
            return .success(.number(number))
        }
        return .failure(.message("unsupported value '\(raw)'"))
    }

    private static func parseString(_ raw: String) -> Result<String, SeyalTOMLError> {
        guard raw.count >= 2, raw.hasPrefix("\""), raw.hasSuffix("\"") else {
            return .failure(.message("unterminated string"))
        }
        let inner = raw.dropFirst().dropLast()
        var result = ""
        var escaped = false
        for character in inner {
            if escaped {
                switch character {
                case "n": result.append("\n")
                case "t": result.append("\t")
                case "\"": result.append("\"")
                case "\\": result.append("\\")
                default: result.append(character)
                }
                escaped = false
            } else if character == "\\" {
                escaped = true
            } else {
                result.append(character)
            }
        }
        if escaped { return .failure(.message("unterminated escape")) }
        return .success(result)
    }

    private static func parseArray(_ raw: String) -> Result<SeyalTOMLValue, SeyalTOMLError> {
        guard raw.hasPrefix("["), raw.hasSuffix("]") else {
            return .failure(.message("unterminated array"))
        }
        let inner = String(raw.dropFirst().dropLast()).trimmingCharacters(in: .whitespaces)
        if inner.isEmpty { return .success(.array([])) }
        var items: [SeyalTOMLValue] = []
        var current = ""
        var inString = false
        var escaped = false
        for character in inner {
            if escaped {
                current.append(character)
                escaped = false
                continue
            }
            if character == "\\" && inString {
                current.append(character)
                escaped = true
                continue
            }
            if character == "\"" {
                inString.toggle()
                current.append(character)
                continue
            }
            if character == "," && !inString {
                switch parseValue(current.trimmingCharacters(in: .whitespaces)) {
                case let .success(value): items.append(value)
                case let .failure(error): return .failure(error)
                }
                current = ""
                continue
            }
            current.append(character)
        }
        if inString { return .failure(.message("unterminated string in array")) }
        if !current.trimmingCharacters(in: .whitespaces).isEmpty {
            switch parseValue(current.trimmingCharacters(in: .whitespaces)) {
            case let .success(value): items.append(value)
            case let .failure(error): return .failure(error)
            }
        }
        return .success(.array(items))
    }
}
