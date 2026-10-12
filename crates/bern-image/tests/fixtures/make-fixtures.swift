// Make the HDR fixtures with ImageIO on macOS 15 or later:
//
//   swiftc -O make-fixtures.swift -o make-fixtures && ./make-fixtures .
//
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let width = 640, height = 360
let linear = CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3)!
var pixels = [Float](repeating: 0, count: width * height * 4)
for y in 0..<height {
    for x in 0..<width {
        let value: Float
        if y < height * 2 / 3 {
            value = x < width / 2 ? 1.0 : 4.0
        } else {
            value = 4.0 * Float(x) / Float(width - 1)
        }
        let i = (y * width + x) * 4
        pixels[i] = value; pixels[i + 1] = value; pixels[i + 2] = value; pixels[i + 3] = 1
    }
}
let data = pixels.withUnsafeBufferPointer { Data(buffer: $0) } as CFData
let provider = CGDataProvider(data: data)!
let hdr = CGImage(width: width, height: height, bitsPerComponent: 32, bitsPerPixel: 128, bytesPerRow: width * 16,
                  space: linear,
                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue),
                  provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!

func write(_ image: CGImage, _ name: String, _ type: UTType, _ options: [CFString: Any]) {
    let url = URL(fileURLWithPath: CommandLine.arguments[1]).appendingPathComponent(name) as CFURL
    let destination = CGImageDestinationCreateWithURL(url, type.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, options as CFDictionary)
    print(name, CGImageDestinationFinalize(destination) ? "written" : "FAILED")
}

// JPEG with an ISO 21496-1 gain map: SDR base plus the HDR boost.
let declared = CGImageCreateCopyWithContentHeadroom(4.0, hdr)!
print("headroom:", declared.contentHeadroom, "source:", hdr.contentHeadroom)
write(declared, "hdr-gainmap.jpg", .jpeg, [kCGImageDestinationEncodeRequest: kCGImageDestinationEncodeToISOGainmap, kCGImageDestinationLossyCompressionQuality: 0.9])
// 16-bit PNG in BT.2100 PQ.
let pqSpace = CGColorSpace(name: CGColorSpace.itur_2100_PQ)!
let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 16, bytesPerRow: 0, space: pqSpace,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder16Little.rawValue)!
ctx.draw(hdr, in: CGRect(x: 0, y: 0, width: width, height: height))
write(ctx.makeImage()!, "hdr-pq.png", .png, [:])
// The same picture as plain SDR, for comparison.
write(hdr, "sdr.png", .png, [kCGImageDestinationEncodeRequest: kCGImageDestinationEncodeToSDR])
