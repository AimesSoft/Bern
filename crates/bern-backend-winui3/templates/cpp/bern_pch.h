// Bern runtime for Windows: the precompiled header. C++/WinRT for the
// front ends the build includes (`BERN_WITH_WINUI`, `BERN_WITH_ISLANDS`) and
// the whole runtime, once in each front end's namespace, so that a build
// compiles only the application's own code. It changes with the runtime,
// the compiler and the SDK, not with the application. Generated build
// input; do not edit.

#pragma once

#include "bern_base.h"

#include <d2d1_3.h>
#include <d2d1effects_2.h>
#include <d3d11.h>
#include <dwmapi.h>
#include <dxgi1_6.h>
#include <winrt/Windows.Graphics.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.h>
#include <winrt/Windows.UI.h>
#include <winrt/Windows.UI.Xaml.Interop.h>

// HDR pictures draw with Direct2D into swap chains.
#pragma comment(lib, "d2d1.lib")
#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")
#pragma comment(lib, "dxguid.lib")

// The native interfaces of the two SwapChainPanels. The WinUI 3 and the
// Windows SDK headers both call theirs ISwapChainPanelNative, with
// different IDs, so a build with both front ends declares them here.
struct __declspec(uuid("63aad0b8-7c24-40ff-85a8-640d944cc325")) IBernWinUISwapChainPanelNative : ::IUnknown {
    virtual HRESULT __stdcall SetSwapChain(IDXGISwapChain* swapChain) = 0;
};
struct __declspec(uuid("f92f19d2-3ade-45a6-a20c-f6f1ea90554b")) IBernIslandsSwapChainPanelNative : ::IUnknown {
    virtual HRESULT __stdcall SetSwapChain(IDXGISwapChain* swapChain) = 0;
};

#if BERN_WITH_WINUI
// The WinUI 3 front end, on the Windows App Runtime 2 the system has.
#include <winrt/Microsoft.UI.h>
#include <winrt/Microsoft.UI.Windowing.h>
#include <winrt/Microsoft.UI.Xaml.h>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Automation.Peers.h>
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
#include <winrt/Windows.UI.Xaml.Automation.Peers.h>
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
