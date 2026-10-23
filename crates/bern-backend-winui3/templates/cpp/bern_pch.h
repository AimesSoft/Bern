// Bern runtime for Windows: the precompiled header. C++/WinRT for the
// front ends the build includes (`BERN_WITH_WINUI`, `BERN_WITH_ISLANDS`) and
// the whole runtime, once in each front end's namespace, so that a build
// compiles only the application's own code. It changes with the runtime,
// the compiler and the SDK, not with the application. Generated build
// input; do not edit.

#pragma once

#include "bern_base.h"
#define BERN_WITH_SVG /*bern:svg-enabled*/
#if BERN_WITH_SVG
#include <WebView2.h>
#include <wrl.h>
#include <winrt/Microsoft.Web.WebView2.Core.h>
#pragma comment(lib, "WebView2LoaderStatic.lib")
#endif

#include <d2d1_3.h>
#include <d2d1effects_2.h>
#include <d3d11.h>
#include <dwmapi.h>
#include <dxgi1_6.h>
#include <shlwapi.h>
#include <shobjidl_core.h>
#include <wincodec.h>
#include <winrt/Windows.ApplicationModel.DataTransfer.h>
#include <winrt/Windows.Devices.Input.h>
#include <winrt/Windows.Graphics.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.h>
#include <winrt/Windows.Storage.Pickers.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.h>
#include <winrt/Windows.UI.h>
#include <winrt/Windows.UI.Input.h>
#include <winrt/Windows.UI.ViewManagement.h>
#include <winrt/Windows.UI.Xaml.Interop.h>

// HDR pictures draw with Direct2D into swap chains; videos show the swap
// chains of media engines.
#pragma comment(lib, "d2d1.lib")
#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")
#pragma comment(lib, "dxguid.lib")
// Drawings and the average color of pictures: Direct2D on WIC bitmaps.
#pragma comment(lib, "windowscodecs.lib")
#pragma comment(lib, "shlwapi.lib")

// The native interfaces of the two SwapChainPanels. The WinUI 3 and the
// Windows SDK headers both call theirs ISwapChainPanelNative, with
// different IDs, so a build with both front ends declares them here.
struct __declspec(uuid("63aad0b8-7c24-40ff-85a8-640d944cc325")) IBernWinUISwapChainPanelNative : ::IUnknown {
    virtual HRESULT __stdcall SetSwapChain(IDXGISwapChain* swapChain) = 0;
};
struct __declspec(uuid("f92f19d2-3ade-45a6-a20c-f6f1ea90554b")) IBernIslandsSwapChainPanelNative : ::IUnknown {
    virtual HRESULT __stdcall SetSwapChain(IDXGISwapChain* swapChain) = 0;
};
struct __declspec(uuid("88fd8248-10da-4810-bb4c-010dd27faea9")) IBernWinUISwapChainPanelNative2 : IBernWinUISwapChainPanelNative {
    virtual HRESULT __stdcall SetSwapChainHandle(HANDLE swapChain) = 0;
};
struct __declspec(uuid("d5a2f60c-37b2-44a2-937b-8d8eb9726821")) IBernIslandsSwapChainPanelNative2 : IBernIslandsSwapChainPanelNative {
    virtual HRESULT __stdcall SetSwapChainHandle(HANDLE swapChain) = 0;
};

#if BERN_WITH_WINUI
// The WinUI 3 front end, on the Windows App Runtime 2 the system has.
#include <winrt/Microsoft.UI.h>
#include <winrt/Microsoft.UI.Composition.h>
#include <winrt/Microsoft.UI.Input.h>
#include <winrt/Microsoft.UI.Windowing.h>
#include <winrt/Microsoft.UI.Xaml.h>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Automation.Peers.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>
#include <winrt/Microsoft.UI.Xaml.Controls.Primitives.h>
#include <winrt/Microsoft.UI.Xaml.Documents.h>
#include <winrt/Microsoft.UI.Xaml.Hosting.h>
#include <winrt/Microsoft.UI.Xaml.Input.h>
#include <winrt/Microsoft.UI.Xaml.Markup.h>
#include <winrt/Microsoft.UI.Xaml.Media.h>
#include <winrt/Microsoft.UI.Xaml.Media.Animation.h>
#include <winrt/Microsoft.UI.Xaml.Media.Imaging.h>
#include <winrt/Microsoft.UI.Xaml.Shapes.h>
#include <winrt/Microsoft.UI.Xaml.XamlTypeInfo.h>
#include <microsoft.ui.xaml.window.h>
#include <winrt/Microsoft.Windows.ApplicationModel.Resources.h>
#endif

#if BERN_WITH_ISLANDS
// The fallback front end, the system XAML of Windows 10 and 11 hosted in
// Win32 windows through XAML Islands.
#include <winrt/Windows.UI.Composition.h>
#include <winrt/Windows.UI.Xaml.h>
#include <winrt/Windows.UI.Xaml.Automation.h>
#include <winrt/Windows.UI.Xaml.Automation.Peers.h>
#include <winrt/Windows.UI.Xaml.Controls.h>
#include <winrt/Windows.UI.Xaml.Controls.Primitives.h>
#include <winrt/Windows.UI.Xaml.Documents.h>
#include <winrt/Windows.UI.Xaml.Hosting.h>
#include <winrt/Windows.UI.Xaml.Input.h>
#include <winrt/Windows.UI.Xaml.Markup.h>
#include <winrt/Windows.UI.Xaml.Media.h>
#include <winrt/Windows.UI.Xaml.Media.Animation.h>
#include <winrt/Windows.UI.Xaml.Media.Imaging.h>
#include <winrt/Windows.UI.Xaml.Shapes.h>
#include <windows.ui.xaml.hosting.desktopwindowxamlsource.h>
#endif

#if BERN_WITH_WINUI
namespace bern::winui {
namespace xaml = winrt::Microsoft::UI::Xaml;
constexpr bool kWinUI = true;
#define BERN_WINUI 1
#include "bern_ui_core.inl"
#include "bern_ui_motion.inl"
#include "bern_ui_drawing.inl"
#include "bern_ui_widgets.inl"
#if BERN_WITH_SVG
#include "bern_ui_svg.inl"
#endif
#include "bern_ui_structure.inl"
#include "bern_ui_features.inl"
#include "bern_ui_scene.inl"
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
#include "bern_ui_motion.inl"
#include "bern_ui_drawing.inl"
#include "bern_ui_widgets.inl"
#if BERN_WITH_SVG
#include "bern_ui_svg.inl"
#endif
#include "bern_ui_structure.inl"
#include "bern_ui_features.inl"
#include "bern_ui_scene.inl"
#include "bern_ui_windows.inl"
#undef BERN_WINUI
}
#endif

#include <winrt/Windows.Data.Json.h>
