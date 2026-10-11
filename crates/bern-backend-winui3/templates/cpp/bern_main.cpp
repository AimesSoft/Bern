// Bern runtime for Windows: the entry point. One instance per user. The
// interface is WinUI 3 when the bootstrapper finds the Windows App Runtime
// on the system, and otherwise the system XAML through XAML Islands, which
// every Windows 10 and 11 has. Nothing is installed or carried along either
// way. Generated build input; do not edit.

#include "bern_base.h"

#include <MddBootstrap.h>

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

int BernRunWinUI();
int BernRunIslands();

static bool Wanted(wchar_t const* name) { return GetEnvironmentVariableW(name, nullptr, 0) != 0; }

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
#if !defined(NDEBUG) && defined(_M_X64)
    SetUnhandledExceptionFilter(ReportCrash);
#endif
    winrt::init_apartment(winrt::apartment_type::single_threaded);
    // A second launch shows the running instance.
    if (!bern::Host::Claim(BERN_BUNDLE_ID)) return 0;
    // `BERN_FORCE_ISLANDS` tries the fallback on systems that have the
    // runtime, during development.
    if (!Wanted(L"BERN_FORCE_ISLANDS")) {
        // Windows App Runtime 1.8, without asking the user to install it
        // when there is none.
        PACKAGE_VERSION minimum{};
        HRESULT found = MddBootstrapInitialize2(0x00010008, L"", minimum, MddBootstrapInitializeOptions_None);
        if (SUCCEEDED(found)) {
            bern::Report("interface: WinUI 3 on the Windows App Runtime");
            int code = BernRunWinUI();
            MddBootstrapShutdown();
            return code;
        }
        char reason[64];
        std::snprintf(reason, sizeof reason, "no Windows App Runtime 1.8 (0x%08lx)", static_cast<unsigned long>(found));
        bern::Report(reason);
    }
    bern::Report("interface: the system XAML in XAML Islands");
    return BernRunIslands();
}
