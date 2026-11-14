// Native pure-Rust animated SVG presenter (`bern-svg` over C ABI).
// Works across macOS, iOS, tvOS, and watchOS without any browser view.
import CoreGraphics

@MainActor
private final class BernSvgController: ObservableObject {
    nonisolated(unsafe) private var player: OpaquePointer?
    private var loadedName: String = ""
    private var image: CGImage?
    private let colorSpace = CGColorSpaceCreateDeviceRGB()
    @Published var active: Bool = true
    @Published var revision: UInt64 = 0

    deinit {
        if let player { bern_svg_free(player) }
    }

    private func ensureLoaded(_ name: String) {
        guard loadedName != name else { return }
        if let player {
            bern_svg_free(player)
            self.player = nil
        }
        loadedName = name
        image = nil
        let keyBytes = Array(name.utf8)
        let alreadyCached = keyBytes.withUnsafeBufferPointer { keyBuf in
            bern_svg_cached(keyBuf.baseAddress, keyBuf.count) != 0
        }
        if alreadyCached {
            player = keyBytes.withUnsafeBufferPointer { keyBuf in
                bern_svg_open(keyBuf.baseAddress, keyBuf.count, nil, 0)
            }
            return
        }
        guard let root = Bundle.main.resourceURL,
              let data = try? Data(contentsOf: root.appendingPathComponent(name)) else {
            NSLog("Bern SVG: unable to load %@", name)
            return
        }
        player = keyBytes.withUnsafeBufferPointer { keyBuf in
            data.withUnsafeBytes { rawBuf in
                let base = rawBuf.bindMemory(to: UInt8.self).baseAddress
                return bern_svg_open(keyBuf.baseAddress, keyBuf.count, base, rawBuf.count)
            }
        }
    }

    func renderFrame(
        name: String,
        date: Date,
        width: CGFloat,
        height: CGFloat,
        scale: CGFloat,
        fit: String,
        playing: Bool,
        speed: Double,
        restart: Double,
        interactive: Bool,
        dark: Bool,
        reduceMotion: Bool
    ) -> CGImage? {
        ensureLoaded(name)
        guard let player else { return nil }
        let fitCode: UInt8 = switch fit {
        case "cover": 1
        case "fill": 2
        default: 0
        }
        let textRgba: UInt32 = dark ? 0xFFFFFFFF : 0x000000FF
        _ = bern_svg_configure(
            player,
            playing ? 1 : 0,
            speed.isFinite && speed > 0 ? speed : 1,
            restart.isFinite ? restart : 0,
            fitCode,
            interactive ? 1 : 0,
            dark ? 1 : 0,
            reduceMotion ? 1 : 0,
            textRgba
        )
        let s = max(1.0, scale)
        let wPx = UInt32(max(1, Int((max(1.0, width) * s).rounded())))
        let hPx = UInt32(max(1, Int((max(1.0, height) * s).rounded())))
        let status = bern_svg_render(player, date.timeIntervalSinceReferenceDate, wPx, hPx, 0)
        let changed = (status & 1) != 0
        let nowActive = (status & 2) != 0

        if changed || image == nil {
            let ptr = bern_svg_pixels_ptr(player)
            let len = bern_svg_pixels_len(player)
            if let ptr, len == Int(wPx) * Int(hPx) * 4 {
                let copy = Data(bytes: ptr, count: len)
                if let provider = CGDataProvider(data: copy as CFData) {
                    let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
                    image = CGImage(
                        width: Int(wPx),
                        height: Int(hPx),
                        bitsPerComponent: 8,
                        bitsPerPixel: 32,
                        bytesPerRow: Int(wPx) * 4,
                        space: colorSpace,
                        bitmapInfo: bitmapInfo,
                        provider: provider,
                        decode: nil,
                        shouldInterpolate: true,
                        intent: .defaultIntent
                    )
                }
            }
        }
        if nowActive != active {
            Task { @MainActor [weak self] in
                guard let self, self.active != nowActive else { return }
                self.active = nowActive
            }
        }
        return image
    }

    func pointer(kind: UInt8, x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat) {
        guard let player else { return }
        let nx = Float(width > 0 ? x / width : 0.5)
        let ny = Float(height > 0 ? y / height : 0.5)
        let status = bern_svg_pointer(player, kind, nx, ny)
        let changed = (status & 1) != 0
        let nowActive = (status & 2) != 0
        if active != nowActive { active = nowActive }
        if changed || nowActive { revision &+= 1 }
    }
}

struct BernSvg: View {
    let name: String
    let label: String?
    let width: CGFloat
    let height: CGFloat
    var fit: String = "contain"
    var playing: Bool = true
    var speed: Double = 1
    var restart: Double = 0
    var interactive: Bool = true
    var data: String = ""
    var externalLinks: Bool = false

    @Environment(\.colorScheme) private var scheme
    @Environment(\.displayScale) private var displayScale
    @Environment(\.scenePhase) private var phase
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @StateObject private var controller = BernSvgController()
    @State private var visible = true

    var body: some View {
        let _ = controller.revision
        #if os(macOS)
        let sceneLive = true
        #else
        let sceneLive = phase != .background
        #endif
        let running = controller.active && playing && visible && sceneLive
        let w = max(0, width)
        let h = max(0, height)
        TimelineView(.animation(minimumInterval: nil, paused: !running)) { timeline in
            if let cgImage = controller.renderFrame(
                name: name,
                date: timeline.date,
                width: w,
                height: h,
                scale: displayScale,
                fit: fit,
                playing: playing && visible && sceneLive,
                speed: speed,
                restart: restart,
                interactive: interactive,
                dark: scheme == .dark,
                reduceMotion: reduceMotion
            ) {
                SwiftUI.Image(decorative: cgImage, scale: max(1.0, displayScale))
                    .interpolation(.high)
                    .resizable()
                    .frame(width: w, height: h)
            } else {
                SwiftUI.Color.clear.frame(width: w, height: h)
            }
        }
        .frame(width: w, height: h)
        .contentShape(SwiftUI.Rectangle())
        .modifier(BernSvgGestures(interactive: interactive, width: w, height: h, controller: controller))
        .clipped()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label ?? "")
        .accessibilityHidden(label == nil)
        .onAppear { visible = true }
        .onDisappear { visible = false }
    }
}

private struct BernSvgGestures: ViewModifier {
    let interactive: Bool
    let width: CGFloat
    let height: CGFloat
    let controller: BernSvgController

    func body(content: Content) -> some View {
        if !interactive {
            content
        } else {
            #if os(tvOS)
            content
                .focusable(true)
                .onTapGesture {
                    controller.pointer(kind: 1, x: width * 0.5, y: height * 0.5, width: width, height: height)
                    controller.pointer(kind: 0, x: width * 0.5, y: height * 0.5, width: width, height: height)
                }
            #elseif os(watchOS)
            content
                .onTapGesture(count: 2) {
                    controller.pointer(kind: 1, x: width * 0.5, y: height * 0.5, width: width, height: height)
                }
                .onTapGesture(count: 1) {
                    controller.pointer(kind: 0, x: width * 0.5, y: height * 0.5, width: width, height: height)
                }
            #else
            content
                .onTapGesture(count: 2) {
                    controller.pointer(kind: 1, x: width * 0.5, y: height * 0.5, width: width, height: height)
                }
                .onTapGesture(count: 1) {
                    controller.pointer(kind: 0, x: width * 0.5, y: height * 0.5, width: width, height: height)
                }
                .onContinuousHover { hover in
                    switch hover {
                    case .active(let pt):
                        controller.pointer(kind: 2, x: pt.x, y: pt.y, width: width, height: height)
                    case .ended:
                        controller.pointer(kind: 3, x: 0, y: 0, width: width, height: height)
                    }
                }
            #endif
        }
    }
}
