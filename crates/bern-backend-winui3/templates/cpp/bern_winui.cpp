// Bern runtime for Windows: the WinUI 3 front end, on the Windows App
// Runtime the bootstrapper found. Generated build input; do not edit.

#include "bern_base.h"

#include <winrt/Windows.Graphics.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.h>
#include <winrt/Windows.UI.h>
#include <winrt/Windows.UI.Xaml.Interop.h>
#include <winrt/Microsoft.UI.h>
#include <winrt/Microsoft.UI.Windowing.h>
#include <winrt/Microsoft.UI.Xaml.h>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>
#include <winrt/Microsoft.UI.Xaml.Controls.Primitives.h>
#include <winrt/Microsoft.UI.Xaml.Input.h>
#include <winrt/Microsoft.UI.Xaml.Markup.h>
#include <winrt/Microsoft.UI.Xaml.Media.h>
#include <winrt/Microsoft.UI.Xaml.Media.Imaging.h>
#include <winrt/Microsoft.UI.Xaml.XamlTypeInfo.h>
#include <microsoft.ui.xaml.window.h>

namespace bern::winui {
namespace xaml = winrt::Microsoft::UI::Xaml;
constexpr bool kWinUI = true;
#define BERN_WINUI 1
#include "bern_ui_core.inl"
#include "bern_ui_widgets.inl"
#include "bern_ui_structure.inl"
#include "bern_ui_windows.inl"
#include "app_ui.inl"
}

int BernRunWinUI() {
    static bern::winui::AppDef const definition{bern::BernCatalog(), bern::winui::Windows()};
    return bern::winui::Run(definition);
}
