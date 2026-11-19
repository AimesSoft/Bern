import SwiftUI
#if os(macOS)
import AppKit
#endif

private func parseIsoDate(_ iso: String) -> Date {
    let fmt = DateFormatter()
    fmt.calendar = Calendar(identifier: .gregorian)
    fmt.locale = Locale(identifier: "en_US_POSIX")
    fmt.timeZone = TimeZone(secondsFromGMT: 0)
    fmt.dateFormat = "yyyy-MM-dd"
    return fmt.date(from: iso) ?? Date()
}

private func formatIsoDate(_ date: Date) -> String {
    let fmt = DateFormatter()
    fmt.calendar = Calendar(identifier: .gregorian)
    fmt.locale = Locale(identifier: "en_US_POSIX")
    fmt.timeZone = TimeZone(secondsFromGMT: 0)
    fmt.dateFormat = "yyyy-MM-dd"
    return fmt.string(from: date)
}

#if os(macOS)
private struct MacGraphicalDatePicker: NSViewRepresentable {
    let date: Date
    let onChange: @MainActor (Date) -> Void

    final class Coordinator: NSObject {
        var onChange: @MainActor (Date) -> Void
        init(onChange: @escaping @MainActor (Date) -> Void) {
            self.onChange = onChange
        }

        @MainActor @objc func changed(_ sender: NSDatePicker) {
            onChange(sender.dateValue)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onChange: onChange)
    }

    func makeNSView(context: Context) -> NSDatePicker {
        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        picker.datePickerElements = .yearMonthDay
        picker.timeZone = TimeZone(secondsFromGMT: 0)
        picker.dateValue = date
        picker.target = context.coordinator
        picker.action = #selector(Coordinator.changed(_:))
        return picker
    }

    func updateNSView(_ picker: NSDatePicker, context: Context) {
        context.coordinator.onChange = onChange
        if formatIsoDate(picker.dateValue) != formatIsoDate(date) {
            picker.dateValue = date
        }
    }
}
#endif

struct CalendarNativeView: View {
    let props: CalendarNativeProps
    let actions: CalendarNativeActions
    let theme: BernNativeTheme

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(verbatim: props.heading)
                    .font(.headline)
                Spacer()
                Text(verbatim: props.date)
                    .font(.subheadline.monospacedDigit())
                    .foregroundStyle(theme.textSecondary)
                Button(props.todayLabel) {
                    actions.today()
                }
            }

            #if os(macOS)
            MacGraphicalDatePicker(
                date: parseIsoDate(props.date),
                onChange: { actions.setDate(formatIsoDate($0)) }
            )
            #else
            DatePicker(
                props.dateLabel,
                selection: Binding(
                    get: { parseIsoDate(props.date) },
                    set: { actions.setDate(formatIsoDate($0)) }
                ),
                displayedComponents: [.date]
            )
            .datePickerStyle(.graphical)
            .labelsHidden()
            #endif
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .stroke(theme.border, lineWidth: 1)
        )
    }
}
