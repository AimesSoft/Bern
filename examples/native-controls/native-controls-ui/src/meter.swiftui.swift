import SwiftUI
#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

#if os(macOS)
private struct MacLevelIndicator: NSViewRepresentable {
    let value: Double
    let min: Double
    let max: Double
    let boosted: Bool
    let onChange: @MainActor (Double) -> Void

    final class Coordinator: NSObject {
        var onChange: @MainActor (Double) -> Void
        init(onChange: @escaping @MainActor (Double) -> Void) {
            self.onChange = onChange
        }

        @MainActor @objc func changed(_ sender: NSLevelIndicator) {
            onChange(sender.doubleValue)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onChange: onChange)
    }

    func makeNSView(context: Context) -> NSLevelIndicator {
        let view = NSLevelIndicator()
        view.levelIndicatorStyle = .discreteCapacity
        view.isEditable = true
        view.minValue = min
        view.maxValue = max
        view.warningValue = max * 0.7
        view.criticalValue = max * 0.9
        view.numberOfTickMarks = 11
        view.target = context.coordinator
        view.action = #selector(Coordinator.changed(_:))
        return view
    }

    func updateNSView(_ view: NSLevelIndicator, context: Context) {
        context.coordinator.onChange = onChange
        view.minValue = min
        view.maxValue = max
        view.doubleValue = value
        view.fillColor = boosted ? NSColor.systemOrange : NSColor.controlAccentColor
    }
}
#endif

struct MeterNativeView: View {
    let props: MeterNativeProps
    let actions: MeterNativeActions
    let theme: BernNativeTheme

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(verbatim: props.heading)
                    .font(.headline)
                Spacer()
                Text(verbatim: props.readout)
                    .font(.subheadline.monospacedDigit())
                    .foregroundStyle(theme.textSecondary)
            }

            HStack(spacing: 16) {
                Gauge(value: props.level, in: props.levelMin...props.levelMax) {
                    Text(verbatim: props.levelLabel)
                } currentValueLabel: {
                    Text(verbatim: String(format: "%.0f", props.level))
                }
                .gaugeStyle(.accessoryCircular)
                .tint(props.boosted ? Color.orange : theme.accent)

                #if os(macOS)
                MacLevelIndicator(
                    value: props.level,
                    min: props.levelMin,
                    max: props.levelMax,
                    boosted: props.boosted,
                    onChange: actions.setLevel
                )
                .frame(height: 28)
                .onContinuousHover { phase in
                    switch phase {
                    case .active(let location):
                        let est = (location.x / 220.0).clamped(to: 0...1) * props.levelMax
                        actions.setHover((est * 10).rounded() / 10)
                    case .ended:
                        actions.setHover(nil)
                    }
                }
                #else
                ProgressView(value: props.level, total: props.levelMax)
                    .tint(props.boosted ? Color.orange : theme.accent)
                #endif

                Stepper(
                    value: Binding(
                        get: { props.level },
                        set: { actions.setLevel($0) }
                    ),
                    in: props.levelMin...props.levelMax,
                    step: 1
                ) {
                    EmptyView()
                }
                .labelsHidden()
            }

            HStack(spacing: 12) {
                Toggle(
                    props.boostedLabel,
                    isOn: Binding(
                        get: { props.boosted },
                        set: { actions.setBoosted($0) }
                    )
                )
                .toggleStyle(.switch)

                Spacer()

                Button(props.resetLabel) {
                    actions.reset()
                }
            }
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .stroke(theme.border, lineWidth: 1)
        )
    }
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
