// Draws background.tiff, the picture behind a disk image's window: a quiet
// surface and one thin arrow, level with the icons (see `src/dmg.rs`).
//
//   swift background.swift
//   tiffutil -cathidpicheck background.png background@2x.png -out background.tiff
//   tiffutil -lzw background.tiff -out background.tiff

import AppKit

let width = 640.0, height = 400.0

func draw(_ scale: Double, _ path: String) {
    let space = CGColorSpace(name: CGColorSpace.sRGB)!
    let context = CGContext(data: nil, width: Int(width * scale), height: Int(height * scale), bitsPerComponent: 8, bytesPerRow: 0,
                            space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.scaleBy(x: scale, y: scale)
    context.setFillColor(CGColor(srgbRed: 0.968, green: 0.968, blue: 0.975, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let y = height - 180, x = width / 2, half = 26.0, head = 8.0
    context.setStrokeColor(CGColor(srgbRed: 0.11, green: 0.11, blue: 0.12, alpha: 0.28))
    context.setLineWidth(1.5)
    context.setLineCap(.round)
    context.setLineJoin(.round)
    context.move(to: CGPoint(x: x - half, y: y))
    context.addLine(to: CGPoint(x: x + half, y: y))
    context.move(to: CGPoint(x: x + half - head, y: y + head))
    context.addLine(to: CGPoint(x: x + half, y: y))
    context.addLine(to: CGPoint(x: x + half - head, y: y - head))
    context.strokePath()
    let image = NSBitmapImageRep(cgImage: context.makeImage()!)
    image.size = NSSize(width: width, height: height)
    try! image.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

draw(1, "background.png")
draw(2, "background@2x.png")
