#!/usr/bin/env swift
//
// make-icon.swift — Generate ClipNoteX.app icon (icon.icns) from scratch.
//
// 構成:
//   1) 1024x1024 のアートを AppKit で描画 (角丸 + グラデーション + 📋 絵文字)
//   2) 16/32/64/128/256/512/1024 の PNG を `icon.iconset/` に出力
//   3) iconutil で .icns に変換 → apps/macos/icon.icns
//
// 使い方:
//   cd apps/macos && swift make-icon.swift

import AppKit
import Foundation

// MARK: - Drawing

func makeIcon(size: CGFloat) -> NSImage {
    let img = NSImage(size: NSSize(width: size, height: size))
    img.lockFocus()
    defer { img.unlockFocus() }

    let rect = NSRect(x: 0, y: 0, width: size, height: size)
    let cornerRadius = size * 0.225 // macOS app-icon-like rounded square
    let path = NSBezierPath(roundedRect: rect, xRadius: cornerRadius, yRadius: cornerRadius)
    path.addClip()

    // Background gradient (deep indigo → bright violet — distinguishable in Dock)
    let grad = NSGradient(starting: NSColor(srgbRed: 0.27, green: 0.30, blue: 0.93, alpha: 1.0),
                          ending: NSColor(srgbRed: 0.55, green: 0.30, blue: 0.95, alpha: 1.0))!
    grad.draw(in: rect, angle: -90)

    // 📋 emoji centered, large
    let emoji = "📋" as NSString
    let fontSize = size * 0.62
    let attrs: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: fontSize),
    ]
    let textSize = emoji.size(withAttributes: attrs)
    let textRect = NSRect(
        x: (size - textSize.width) / 2,
        y: (size - textSize.height) / 2 - size * 0.02,
        width: textSize.width,
        height: textSize.height
    )
    emoji.draw(in: textRect, withAttributes: attrs)

    return img
}

func writePNG(_ img: NSImage, size: CGFloat, to url: URL) throws {
    guard let tiff = img.tiffRepresentation,
          let rep = NSBitmapImageRep(data: tiff) else {
        throw NSError(domain: "icon", code: 1)
    }
    rep.size = NSSize(width: size, height: size)
    guard let data = rep.representation(using: .png, properties: [:]) else {
        throw NSError(domain: "icon", code: 2)
    }
    try data.write(to: url)
}

// MARK: - Main

let cwd = FileManager.default.currentDirectoryPath
let here = URL(fileURLWithPath: cwd)
let isetDir = here.appendingPathComponent("icon.iconset")

try? FileManager.default.removeItem(at: isetDir)
try FileManager.default.createDirectory(at: isetDir, withIntermediateDirectories: true)

// Apple naming convention: icon_<size>x<size>[@2x].png
let entries: [(name: String, size: CGFloat)] = [
    ("icon_16x16.png",      16),
    ("icon_16x16@2x.png",   32),
    ("icon_32x32.png",      32),
    ("icon_32x32@2x.png",   64),
    ("icon_128x128.png",   128),
    ("icon_128x128@2x.png",256),
    ("icon_256x256.png",   256),
    ("icon_256x256@2x.png",512),
    ("icon_512x512.png",   512),
    ("icon_512x512@2x.png",1024),
]

for (name, size) in entries {
    let img = makeIcon(size: size)
    let url = isetDir.appendingPathComponent(name)
    try writePNG(img, size: size, to: url)
    print("✔ \(name)")
}

// Convert to .icns via iconutil
let task = Process()
task.launchPath = "/usr/bin/iconutil"
task.arguments = ["-c", "icns", isetDir.path, "-o", here.appendingPathComponent("icon.icns").path]
try task.run()
task.waitUntilExit()

if task.terminationStatus == 0 {
    print("\n✅ icon.icns written")
    try? FileManager.default.removeItem(at: isetDir)
} else {
    print("⚠ iconutil failed (status \(task.terminationStatus)); keeping icon.iconset/")
}
