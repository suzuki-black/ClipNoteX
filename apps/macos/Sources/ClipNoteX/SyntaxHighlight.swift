// SyntaxHighlight.swift — lightweight regex-based syntax highlighter.
//
// We avoid pulling in a full tokenizer (e.g. Splash, Sourceful) to keep
// the Swift target dependency-free. The goal here isn't "perfect code
// rendering" but "easier to skim than a wall of monochrome text" — JSON
// strings green, keywords purple, comments grey, numbers blue.
//
// Heuristics are intentionally simple and may misclassify edge cases.

import AppKit

enum SyntaxHighlight {

    static func highlight(_ text: String, language: String) -> NSAttributedString {
        let attr = NSMutableAttributedString(
            string: text,
            attributes: [
                .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
                .foregroundColor: NSColor.labelColor,
            ]
        )
        let full = NSRange(location: 0, length: (text as NSString).length)
        let palette = Palette.current()

        let lang = language.lowercased()
        switch lang {
        case "json":
            applyJSON(attr, full: full, palette: palette)
        case "sql":
            applySQL(attr, full: full, palette: palette)
        case "javascript", "typescript", "js", "ts":
            applyJSFamily(attr, full: full, palette: palette)
        case "html":
            applyHTML(attr, full: full, palette: palette)
        case "css":
            applyCSS(attr, full: full, palette: palette)
        case "markdown", "md":
            applyMarkdown(attr, full: full, palette: palette)
        default:
            break // plain text — leave default attributes
        }
        return attr
    }

    // MARK: - Palette (adapts to light/dark)

    private struct Palette {
        let keyword: NSColor
        let string: NSColor
        let number: NSColor
        let comment: NSColor
        let key: NSColor
        let symbol: NSColor

        static func current() -> Palette {
            // 簡易: ダークモード前提の彩度高め。light でも視認できる程度の濃さに調整。
            Palette(
                keyword: NSColor(srgbRed: 0.85, green: 0.45, blue: 0.95, alpha: 1),  // purple
                string:  NSColor(srgbRed: 0.55, green: 0.85, blue: 0.45, alpha: 1),  // green
                number:  NSColor(srgbRed: 0.45, green: 0.75, blue: 1.00, alpha: 1),  // blue
                comment: NSColor(srgbRed: 0.50, green: 0.55, blue: 0.55, alpha: 1),  // grey
                key:     NSColor(srgbRed: 1.00, green: 0.70, blue: 0.35, alpha: 1),  // orange
                symbol:  NSColor(srgbRed: 0.95, green: 0.50, blue: 0.55, alpha: 1)   // red
            )
        }
    }

    // MARK: - Helpers

    private static func apply(_ attr: NSMutableAttributedString,
                              pattern: String,
                              options: NSRegularExpression.Options = [],
                              full: NSRange,
                              group: Int = 0,
                              color: NSColor) {
        guard let re = try? NSRegularExpression(pattern: pattern, options: options) else { return }
        let nsStr = attr.string as NSString
        re.enumerateMatches(in: attr.string, options: [], range: full) { match, _, _ in
            guard let m = match else { return }
            let g = group < m.numberOfRanges ? m.range(at: group) : m.range
            if g.location != NSNotFound, g.length > 0, g.upperBound <= nsStr.length {
                attr.addAttribute(.foregroundColor, value: color, range: g)
            }
        }
    }

    private static func applyKeywords(_ attr: NSMutableAttributedString, full: NSRange,
                                      list: [String], color: NSColor, caseInsensitive: Bool = false) {
        let pattern = "\\b(" + list.joined(separator: "|") + ")\\b"
        let opts: NSRegularExpression.Options = caseInsensitive ? [.caseInsensitive] : []
        apply(attr, pattern: pattern, options: opts, full: full, group: 0, color: color)
    }

    // MARK: - JSON

    private static func applyJSON(_ attr: NSMutableAttributedString, full: NSRange, palette: Palette) {
        // keys (string before :)
        apply(attr, pattern: #""(\\.|[^"\\])*"\s*:"#, full: full, color: palette.key)
        // strings (after colon or in arrays)
        apply(attr, pattern: #":\s*"(\\.|[^"\\])*""#, full: full, color: palette.string)
        // standalone strings inside arrays
        apply(attr, pattern: #"\[(?:[^\]]*?)"(\\.|[^"\\])*""#, full: full, color: palette.string)
        // numbers
        apply(attr, pattern: #"\b-?\d+(\.\d+)?([eE][+-]?\d+)?\b"#, full: full, color: palette.number)
        // literals
        applyKeywords(attr, full: full, list: ["true", "false", "null"], color: palette.keyword)
    }

    // MARK: - SQL

    private static func applySQL(_ attr: NSMutableAttributedString, full: NSRange, palette: Palette) {
        let keywords = ["SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "INSERT", "INTO", "VALUES",
                        "UPDATE", "SET", "DELETE", "JOIN", "INNER", "LEFT", "RIGHT", "OUTER",
                        "ON", "AS", "GROUP", "BY", "ORDER", "HAVING", "LIMIT", "OFFSET",
                        "CREATE", "TABLE", "DROP", "ALTER", "ADD", "COLUMN", "PRIMARY", "KEY",
                        "FOREIGN", "REFERENCES", "INDEX", "UNIQUE", "NULL", "DEFAULT",
                        "CASE", "WHEN", "THEN", "ELSE", "END", "DISTINCT", "UNION", "ALL",
                        "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "WITH", "RECURSIVE", "IF",
                        "EXISTS", "TRUE", "FALSE"]
        applyKeywords(attr, full: full, list: keywords, color: palette.keyword, caseInsensitive: true)
        // strings 'single' and "double"
        apply(attr, pattern: #"'(\\.|[^'\\])*'"#, full: full, color: palette.string)
        apply(attr, pattern: #""(\\.|[^"\\])*""#, full: full, color: palette.string)
        // numbers
        apply(attr, pattern: #"\b\d+(\.\d+)?\b"#, full: full, color: palette.number)
        // line comments
        apply(attr, pattern: "--.*", full: full, color: palette.comment)
        // block comments
        apply(attr, pattern: #"/\*[\s\S]*?\*/"#, full: full, color: palette.comment)
    }

    // MARK: - JS / TS

    private static func applyJSFamily(_ attr: NSMutableAttributedString, full: NSRange, palette: Palette) {
        let keywords = ["function", "var", "let", "const", "return", "if", "else", "for", "while",
                        "do", "switch", "case", "break", "continue", "new", "delete", "typeof",
                        "instanceof", "in", "of", "this", "class", "extends", "super", "import",
                        "export", "from", "as", "default", "async", "await", "yield", "try",
                        "catch", "finally", "throw", "true", "false", "null", "undefined",
                        "interface", "type", "enum", "implements", "private", "public", "protected",
                        "static", "readonly", "abstract", "void", "any", "number", "string", "boolean"]
        applyKeywords(attr, full: full, list: keywords, color: palette.keyword)
        // strings (single, double, backtick)
        apply(attr, pattern: #""(\\.|[^"\\])*""#, full: full, color: palette.string)
        apply(attr, pattern: #"'(\\.|[^'\\])*'"#, full: full, color: palette.string)
        apply(attr, pattern: #"`(\\.|[^`\\])*`"#, full: full, color: palette.string)
        // numbers
        apply(attr, pattern: #"\b\d+(\.\d+)?\b"#, full: full, color: palette.number)
        // line comments
        apply(attr, pattern: #"//.*"#, full: full, color: palette.comment)
        // block comments
        apply(attr, pattern: #"/\*[\s\S]*?\*/"#, full: full, color: palette.comment)
    }

    // MARK: - HTML

    private static func applyHTML(_ attr: NSMutableAttributedString, full: NSRange, palette: Palette) {
        // entire tag including attributes
        apply(attr, pattern: #"<\/?[A-Za-z][^>]*>"#, full: full, color: palette.symbol)
        // attribute values
        apply(attr, pattern: #"=\s*"[^"]*""#, full: full, color: palette.string)
        apply(attr, pattern: #"=\s*'[^']*'"#, full: full, color: palette.string)
        // comments
        apply(attr, pattern: #"<!--[\s\S]*?-->"#, full: full, color: palette.comment)
    }

    // MARK: - CSS

    private static func applyCSS(_ attr: NSMutableAttributedString, full: NSRange, palette: Palette) {
        // selectors before {
        apply(attr, pattern: #"^[^{}\n]+(?=\s*\{)"#, options: [.anchorsMatchLines], full: full, color: palette.key)
        // properties
        apply(attr, pattern: #"\b[a-z-]+(?=\s*:)"#, full: full, color: palette.keyword)
        // string values
        apply(attr, pattern: #""[^"]*""#, full: full, color: palette.string)
        apply(attr, pattern: #"'[^']*'"#, full: full, color: palette.string)
        // numbers + units
        apply(attr, pattern: #"\b\d+(\.\d+)?(px|em|rem|%|vh|vw|s|ms|deg)?\b"#, full: full, color: palette.number)
        // comments
        apply(attr, pattern: #"/\*[\s\S]*?\*/"#, full: full, color: palette.comment)
    }

    // MARK: - Markdown

    private static func applyMarkdown(_ attr: NSMutableAttributedString, full: NSRange, palette: Palette) {
        // headings
        apply(attr, pattern: #"^#{1,6}\s+.*"#, options: [.anchorsMatchLines], full: full, color: palette.key)
        // bold
        apply(attr, pattern: #"\*\*[^*]+\*\*"#, full: full, color: palette.keyword)
        // italic
        apply(attr, pattern: #"(?<!\*)\*[^*]+\*(?!\*)"#, full: full, color: palette.keyword)
        // inline code
        apply(attr, pattern: #"`[^`]+`"#, full: full, color: palette.string)
        // fenced code blocks
        apply(attr, pattern: #"```[\s\S]*?```"#, full: full, color: palette.string)
        // links
        apply(attr, pattern: #"\[[^\]]+\]\([^)]+\)"#, full: full, color: palette.symbol)
        // list bullets
        apply(attr, pattern: #"^\s*[-*+]\s+"#, options: [.anchorsMatchLines], full: full, color: palette.symbol)
    }
}
