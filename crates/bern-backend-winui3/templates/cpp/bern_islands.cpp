// Bern runtime for Windows: the fallback front end, the system XAML of
// Windows 10 and 11 hosted in Win32 windows through XAML Islands. Generated
// build input; do not edit.

#include "bern_base.h"

#include <dwmapi.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.h>
#include <winrt/Windows.UI.h>
#include <winrt/Windows.UI.Xaml.h>
#include <winrt/Windows.UI.Xaml.Automation.h>
#include <winrt/Windows.UI.Xaml.Controls.h>
#include <winrt/Windows.UI.Xaml.Controls.Primitives.h>
#include <winrt/Windows.UI.Xaml.Hosting.h>
#include <winrt/Windows.UI.Xaml.Input.h>
#include <winrt/Windows.UI.Xaml.Markup.h>
#include <winrt/Windows.UI.Xaml.Media.h>
#include <winrt/Windows.UI.Xaml.Media.Imaging.h>
#include <windows.ui.xaml.hosting.desktopwindowxamlsource.h>

namespace bern::islands {
namespace xaml = winrt::Windows::UI::Xaml;
constexpr bool kWinUI = false;
#define BERN_WINUI 0
#include "bern_ui_core.inl"
#include "bern_ui_widgets.inl"
#include "bern_ui_structure.inl"
#include "bern_ui_windows.inl"
#include "app_ui.inl"
}

int BernRunIslands() {
    static bern::islands::AppDef const definition{bern::BernCatalog(), bern::islands::Windows()};
    return bern::islands::Run(definition);
}
