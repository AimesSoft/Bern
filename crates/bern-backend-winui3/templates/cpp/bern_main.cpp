// Bern runtime for Windows: the entry point. One instance per user. With
// both front ends (the default) the interface is WinUI 3 when the system
// has a Windows App Runtime 2, and otherwise the system XAML through XAML
// Islands, which every Windows 10 and 11 has; nothing is installed or
// carried along either way. A build with WinUI 3 alone offers the runtime's
// installer when there is none; one with the system XAML alone never looks
// for the runtime. Generated build input; do not edit.

// Through the precompiled header, which starts with bern_base.h.
#include "bern_pch.h"

#if BERN_WITH_WINUI
#include <MddBootstrap.h>
#include <appmodel.h>
#include <shellapi.h>
#endif

#include "app_id.h"

#if !defined(NDEBUG) && defined(_M_X64)
#include <dbghelp.h>

// Debug builds: an unhandled exception prints the stack, with functions and
// lines from the PDB next to the executable, on standard error.
static LONG WINAPI ReportCrash(EXCEPTION_POINTERS* info) {
    HANDLE process = GetCurrentProcess();
    SymSetOptions(SYMOPT_LOAD_LINES | SYMOPT_UNDNAME | SYMOPT_DEFERRED_LOADS);
    SymInitialize(process, nullptr, TRUE);
    CONTEXT context = *info->ContextRecord;
    STACKFRAME64 frame{};
    frame.AddrPC.Offset = context.Rip;
    frame.AddrPC.Mode = AddrModeFlat;
    frame.AddrFrame.Offset = context.Rbp;
    frame.AddrFrame.Mode = AddrModeFlat;
    frame.AddrStack.Offset = context.Rsp;
    frame.AddrStack.Mode = AddrModeFlat;
    std::fprintf(stderr, "bern: crash 0x%08lx at %p\n", static_cast<unsigned long>(info->ExceptionRecord->ExceptionCode),
        info->ExceptionRecord->ExceptionAddress);
    for (int depth = 0; depth < 48; ++depth) {
        if (!StackWalk64(IMAGE_FILE_MACHINE_AMD64, process, GetCurrentThread(), &frame, &context, nullptr, SymFunctionTableAccess64,
                SymGetModuleBase64, nullptr))
            break;
        alignas(SYMBOL_INFO) char buffer[sizeof(SYMBOL_INFO) + 512];
        auto symbol = reinterpret_cast<SYMBOL_INFO*>(buffer);
        symbol->SizeOfStruct = sizeof(SYMBOL_INFO);
        symbol->MaxNameLen = 511;
        DWORD64 displacement = 0;
        IMAGEHLP_LINE64 line{sizeof(line)};
        DWORD column = 0;
        bool named = SymFromAddr(process, frame.AddrPC.Offset, &displacement, symbol);
        bool located = SymGetLineFromAddr64(process, frame.AddrPC.Offset, &column, &line);
        std::fprintf(stderr, "bern:   %s", named ? symbol->Name : "?");
        if (located) std::fprintf(stderr, " (%s:%lu)", line.FileName, static_cast<unsigned long>(line.LineNumber));
        std::fprintf(stderr, "\n");
    }
    std::fflush(stderr);
    return EXCEPTION_CONTINUE_SEARCH;
}
#endif

static bool Wanted(wchar_t const* name) { return GetEnvironmentVariableW(name, nullptr, 0) != 0; }

#if BERN_WITH_ISLANDS
int BernRunIslands();
#endif

#if BERN_WITH_WINUI
int BernRunWinUI(std::wstring const& resources);

// The Windows App Runtime 2. The application is built against the newest
// SDK and uses only what every 2.x has, so any 2.x runs it: 2.x releases
// share one framework package and stay compatible.
constexpr UINT32 kRuntimeRelease = 0x00020000;

static PACKAGE_VERSION RuntimeMinimum() {
    PACKAGE_VERSION version{};
    version.Major = 2;
    return version;
}
// The copy Windows 11 ships with, for its own components; ordinary
// programs reach it through the system's dynamic dependencies.
constexpr wchar_t kInboxRuntime[] = L"Microsoft.WindowsAppRuntime.CBS.2_8wekyb3d8bbwe";

/// `BERN_APP_RUNTIME=installed|inbox|none` limits where the runtime may
/// come from, during development.
static bool Allowed(wchar_t const* source) {
    wchar_t value[16] = {};
    if (GetEnvironmentVariableW(L"BERN_APP_RUNTIME", value, 16) == 0) return true;
    return std::wcscmp(value, source) == 0;
}

static void ReportFailure(char const* what, HRESULT result) {
    char reason[96];
    std::snprintf(reason, sizeof reason, "%s (0x%08lx)", what, static_cast<unsigned long>(result));
    bern::Report(reason);
}

/// A Windows App Runtime 2 another program installed, through the
/// bootstrapper, which also serves Windows 10. Never asks to install one.
static bool UseInstalledRuntime() {
    if (!Allowed(L"installed")) return false;
    HRESULT found = MddBootstrapInitialize2(kRuntimeRelease, L"", RuntimeMinimum(), MddBootstrapInitializeOptions_None);
    if (FAILED(found)) ReportFailure("no installed Windows App Runtime 2", found);
    return SUCCEEDED(found);
}

/// The runtime that comes with Windows 11, through the system's dynamic
/// dependencies, which Windows 10 does not have.
static bool UseInboxRuntime() {
    if (!Allowed(L"inbox")) return false;
    HMODULE kernelbase = GetModuleHandleW(L"kernelbase.dll");
    auto tryCreate = reinterpret_cast<decltype(&TryCreatePackageDependency)>(GetProcAddress(kernelbase, "TryCreatePackageDependency"));
    auto add = reinterpret_cast<decltype(&AddPackageDependency)>(GetProcAddress(kernelbase, "AddPackageDependency"));
    if (!tryCreate || !add) return false;
#if defined(_M_ARM64)
    auto architecture = PackageDependencyProcessorArchitectures_Arm64;
#else
    auto architecture = PackageDependencyProcessorArchitectures_X64;
#endif
    PWSTR id = nullptr;
    HRESULT created = tryCreate(nullptr, kInboxRuntime, RuntimeMinimum(), architecture, PackageDependencyLifetimeKind_Process,
        nullptr, CreatePackageDependencyOptions_None, &id);
    PACKAGEDEPENDENCY_CONTEXT context{};
    PWSTR name = nullptr;
    HRESULT added = SUCCEEDED(created) ? add(id, 0, AddPackageDependencyOptions_None, &context, &name) : created;
    if (id) HeapFree(GetProcessHeap(), 0, id);
    if (name) HeapFree(GetProcessHeap(), 0, name);
    if (FAILED(added)) ReportFailure("no Windows App Runtime 2 with Windows", added);
    return SUCCEEDED(added);
}

/// The folder the WinUI 3 library comes from: the runtime package in use.
static std::wstring RuntimeFolder() {
    wchar_t path[MAX_PATH] = {};
    if (HMODULE xaml = LoadLibraryW(L"Microsoft.ui.xaml.dll")) GetModuleFileNameW(xaml, path, MAX_PATH);
    std::wstring folder(path);
    return folder.substr(0, folder.find_last_of(L'\\'));
}

/// WinUI 3 on a Windows App Runtime 2 the system has, if it has one: the
/// one another program installed, else the one that comes with Windows.
static std::optional<int> RunWinUI() {
    bool bootstrapped = UseInstalledRuntime();
    bool inbox = !bootstrapped && UseInboxRuntime();
    if (!bootstrapped && !inbox) return std::nullopt;
    auto folder = RuntimeFolder();
    bern::Report("interface: WinUI 3 on " + winrt::to_string(folder.substr(folder.find_last_of(L'\\') + 1)));
    // The copy that comes with Windows does not lead WinUI to its own
    // resources; the application points at them.
    int code = BernRunWinUI(inbox ? folder + L"\\resources.pri" : std::wstring());
    if (bootstrapped) MddBootstrapShutdown();
    return code;
}
#endif

#if BERN_WITH_WINUI && !BERN_WITH_ISLANDS
/// A build with WinUI 3 alone, on a system without the runtime: say so in
/// the interface language and offer the installer of the newest release in
/// the browser. The application then ends; it starts once the runtime is
/// installed.
static int OfferRuntime() {
    // The text tables keep pointing into the catalog, so it has to live on.
    static bern::Catalog const catalog = bern::BernCatalog();
    bern::Text::Install(catalog);
    auto title = bern::Text::Tr("app.name");
    auto message = bern::Text::Tr("bern.runtime.missing");
    if (MessageBoxW(nullptr, message.c_str(), title.c_str(), MB_YESNO | MB_ICONINFORMATION | MB_SETFOREGROUND) == IDYES) {
        ShellExecuteW(nullptr, L"open", BERN_RUNTIME_INSTALLER, nullptr, nullptr, SW_SHOWNORMAL);
    }
    return 1;
}
#endif

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
#if !defined(NDEBUG) && defined(_M_X64)
    SetUnhandledExceptionFilter(ReportCrash);
#endif
    winrt::init_apartment(winrt::apartment_type::single_threaded);
    // A second launch shows the running instance.
    if (!bern::Host::Claim(BERN_BUNDLE_ID)) return 0;
#if BERN_WITH_WINUI
    // `BERN_FORCE_ISLANDS` tries the fallback on systems that have the
    // runtime, during development.
    if (!(BERN_WITH_ISLANDS && Wanted(L"BERN_FORCE_ISLANDS"))) {
        if (auto code = RunWinUI()) return *code;
    }
#endif
#if BERN_WITH_ISLANDS
    bern::Report("interface: the system XAML in XAML Islands");
    return BernRunIslands();
#else
    return OfferRuntime();
#endif
}
