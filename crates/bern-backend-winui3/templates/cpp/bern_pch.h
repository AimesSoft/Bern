// Bern runtime for Windows: the precompiled header. C++/WinRT for the
// front ends the build includes (`BERN_WITH_WINUI`, `BERN_WITH_ISLANDS`) and
// the whole runtime, once in each front end's namespace, so that a build
// compiles only the application's own code. It changes with the runtime,
// the compiler and the SDK, not with the application. Generated build
// input; do not edit.

#pragma once

#include "bern_base.h"

#include <dwmapi.h>
#include <winrt/Windows.Graphics.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.h>
#include <winrt/Windows.UI.h>
#include <winrt/Windows.UI.Xaml.Interop.h>

#if BERN_WITH_WINUI
// The WinUI 3 front end, on the Windows App Runtime 2 the system has.
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
#include <winrt/Microsoft.Windows.ApplicationModel.Resources.h>
#endif

#if BERN_WITH_ISLANDS
// The fallback front end, the system XAML of Windows 10 and 11 hosted in
// Win32 windows through XAML Islands.
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
#endif

#if BERN_WITH_WINUI
namespace bern::winui {
namespace xaml = winrt::Microsoft::UI::Xaml;
constexpr bool kWinUI = true;
#define BERN_WINUI 1
#include "bern_ui_core.inl"
#include "bern_ui_widgets.inl"
#include "bern_ui_structure.inl"
#include "bern_ui_windows.inl"
#undef BERN_WINUI
}
#endif

#if BERN_WITH_ISLANDS
namespace bern::islands {
namespace xaml = winrt::Windows::UI::Xaml;
constexpr bool kWinUI = false;
#define BERN_WINUI 0
#include "bern_ui_core.inl"
#include "bern_ui_widgets.inl"
#include "bern_ui_structure.inl"
#include "bern_ui_windows.inl"
#undef BERN_WINUI
}
#endif
