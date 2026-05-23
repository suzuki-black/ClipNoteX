// ExclusionTableController.swift — editable list of exclusion rules.
//
// 2-column NSTableView:
//   Match (popup: bundle_id | exe_basename | window_title)
//   Value (text, editable)
//
// All changes write back to Settings.exclusionsJSON immediately, which
// in turn calls `cnx_set_exclusions_json` to update the running filter.

import AppKit
import Foundation

private struct Rule: Codable {
    var match: String // "bundle_id" | "exe_basename" | "window_title"
    var value: String

    init(match: String, value: String) {
        self.match = match
        self.value = value
    }

    enum CodingKeys: String, CodingKey { case match, value }
}

private let matchKinds = ["bundle_id", "exe_basename", "window_title"]

final class ExclusionTableController: NSObject, NSTableViewDataSource, NSTableViewDelegate {

    let tableView: NSTableView
    private var rules: [Rule] = []

    override init() {
        let tv = NSTableView()
        tv.allowsMultipleSelection = false
        tv.headerView = NSTableHeaderView()
        tv.usesAlternatingRowBackgroundColors = true
        tv.rowHeight = 22
        self.tableView = tv
        super.init()

        let colMatch = NSTableColumn(identifier: .init("match"))
        colMatch.title = "Match"
        colMatch.width = 120
        colMatch.minWidth = 90
        tv.addTableColumn(colMatch)

        let colValue = NSTableColumn(identifier: .init("value"))
        colValue.title = "Value (bundle ID, executable name or window title)"
        colValue.width = 320
        colValue.minWidth = 200
        tv.addTableColumn(colValue)

        tv.dataSource = self
        tv.delegate = self

        loadFromSettings()
    }

    // MARK: - Persistence

    private func loadFromSettings() {
        let json = Settings.exclusionsJSON
        if let data = json.data(using: .utf8),
           let parsed = try? JSONDecoder().decode([Rule].self, from: data) {
            rules = parsed
        } else {
            rules = []
        }
        tableView.reloadData()
    }

    private func saveToSettings() {
        guard let data = try? JSONEncoder().encode(rules),
              let json = String(data: data, encoding: .utf8) else { return }
        Settings.exclusionsJSON = json
    }

    // MARK: - Mutation actions

    @objc func addEmpty() {
        rules.append(Rule(match: "bundle_id", value: ""))
        tableView.reloadData()
        let row = rules.count - 1
        tableView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
        tableView.editColumn(1, row: row, with: nil, select: true)
        saveToSettings()
    }

    @objc func removeSelected() {
        let row = tableView.selectedRow
        guard row >= 0, row < rules.count else { return }
        rules.remove(at: row)
        tableView.reloadData()
        saveToSettings()
    }

    @objc func resetToDefaults() {
        Settings.exclusionsJSON = Settings.defaultExclusionsJSON
        loadFromSettings()
    }

    // MARK: - NSTableViewDataSource

    func numberOfRows(in tableView: NSTableView) -> Int { rules.count }

    // MARK: - NSTableViewDelegate

    func tableView(_ tableView: NSTableView,
                   viewFor tableColumn: NSTableColumn?,
                   row: Int) -> NSView? {
        guard let col = tableColumn, row < rules.count else { return nil }
        let rule = rules[row]

        if col.identifier.rawValue == "match" {
            let popup = NSPopUpButton()
            popup.addItems(withTitles: matchKinds)
            popup.selectItem(withTitle: rule.match)
            popup.target = self
            popup.action = #selector(matchChanged(_:))
            popup.tag = row
            return wrap(popup)
        } else {
            let field = NSTextField()
            field.stringValue = rule.value
            field.isBordered = false
            field.drawsBackground = false
            field.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
            field.target = self
            field.action = #selector(valueChanged(_:))
            field.tag = row
            return wrap(field)
        }
    }

    private func wrap(_ v: NSView) -> NSTableCellView {
        let cell = NSTableCellView()
        cell.addSubview(v)
        v.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            v.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 2),
            v.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -2),
            v.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
        ])
        return cell
    }

    @objc private func matchChanged(_ sender: NSPopUpButton) {
        let row = sender.tag
        guard row < rules.count, let title = sender.selectedItem?.title else { return }
        rules[row].match = title
        saveToSettings()
    }

    @objc private func valueChanged(_ sender: NSTextField) {
        let row = sender.tag
        guard row < rules.count else { return }
        rules[row].value = sender.stringValue
        saveToSettings()
    }
}
