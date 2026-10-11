// Bern installer. Puts the application in the user's own programs folder
// (%LOCALAPPDATA%\Programs), with a Start menu shortcut and an entry in
// Settings > Apps, and removes it again. No administrator rights; nothing
// is downloaded; the files travel inside this program as resources.
//
//   setup.exe              install, asking first
//   setup.exe /S           install silently
//   uninstall.exe          remove, asking first (a copy of this program in
//                          the installed folder)
//   uninstall.exe /S       remove silently
//
// Generated build input; do not edit.

#include <windows.h>
#include <knownfolders.h>
#include <objbase.h>
#include <shellapi.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <tlhelp32.h>

#include <cwchar>
#include <string>

#include "setup_info.h"

namespace {

bool silent = false;

/// The texts in the user's interface language, else English.
Texts const& Text() {
    LANGID language = GetUserDefaultUILanguage();
    WORD primary = PRIMARYLANGID(language);
    WORD sub = SUBLANGID(language);
    wchar_t const* tag = L"en";
    if (primary == LANG_CHINESE) {
        bool traditional = sub == SUBLANG_CHINESE_TRADITIONAL || sub == SUBLANG_CHINESE_HONGKONG || sub == SUBLANG_CHINESE_MACAU;
        tag = traditional ? L"zh-Hant" : L"zh-Hans";
    } else if (primary == LANG_JAPANESE) {
        tag = L"ja";
    }
    for (auto const& texts : kTexts)
        if (std::wcscmp(texts.language, tag) == 0) return texts;
    return kTexts[0];
}

/// `text` with `{name}` and `{version}` filled in.
std::wstring Fill(wchar_t const* text) {
    std::wstring out(text);
    for (auto [key, value] : {std::pair{L"{name}", Text().name}, std::pair{L"{version}", kVersion}}) {
        std::wstring const pattern(key);
        for (size_t at = out.find(pattern); at != std::wstring::npos; at = out.find(pattern, at + std::wcslen(value)))
            out.replace(at, pattern.size(), value);
    }
    return out;
}

std::wstring Title() { return Fill(Text().title); }

bool Ask(wchar_t const* text) {
    return silent || MessageBoxW(nullptr, Fill(text).c_str(), Title().c_str(), MB_YESNO | MB_ICONQUESTION | MB_SETFOREGROUND) == IDYES;
}

void Tell(wchar_t const* text, UINT icon = MB_ICONINFORMATION) {
    if (!silent) MessageBoxW(nullptr, Fill(text).c_str(), Title().c_str(), MB_OK | icon | MB_SETFOREGROUND);
}

std::wstring KnownFolder(KNOWNFOLDERID const& id) {
    PWSTR path = nullptr;
    std::wstring out;
    if (SUCCEEDED(SHGetKnownFolderPath(id, KF_FLAG_CREATE, nullptr, &path))) out = path;
    CoTaskMemFree(path);
    return out;
}

std::wstring InstallFolder() { return KnownFolder(FOLDERID_UserProgramFiles) + L"\\" + kFolder; }
std::wstring Shortcut() { return KnownFolder(FOLDERID_Programs) + L"\\" + kShortcut + L".lnk"; }
std::wstring UninstallKey() { return std::wstring(L"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\") + kKey; }

std::wstring ThisProgram() {
    wchar_t path[MAX_PATH] = {};
    GetModuleFileNameW(nullptr, path, MAX_PATH);
    return path;
}

std::wstring FolderOf(std::wstring const& path) { return path.substr(0, path.find_last_of(L'\\')); }

/// Remove everything in a folder but `keep` (a file directly in it), and
/// the folder itself when nothing is kept. False when something stayed.
bool RemoveTree(std::wstring const& folder, wchar_t const* keep = nullptr) {
    WIN32_FIND_DATAW entry;
    HANDLE find = FindFirstFileW((folder + L"\\*").c_str(), &entry);
    bool removed = true;
    if (find != INVALID_HANDLE_VALUE) {
        do {
            std::wstring name = entry.cFileName;
            if (name == L"." || name == L".." || (keep && _wcsicmp(name.c_str(), keep) == 0)) continue;
            std::wstring path = folder + L"\\" + name;
            if (entry.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
                removed = RemoveTree(path) && removed;
            } else {
                SetFileAttributesW(path.c_str(), FILE_ATTRIBUTE_NORMAL);
                removed = DeleteFileW(path.c_str()) && removed;
            }
        } while (FindNextFileW(find, &entry));
        FindClose(find);
    }
    return keep ? removed : RemoveDirectoryW(folder.c_str()) && removed;
}

/// Whether the installed application or one of its tools runs: a process
/// whose program is in the folder, other than the uninstaller handing over.
bool Running(std::wstring const& folder) {
    std::wstring const prefix = folder + L"\\";
    std::wstring const uninstaller = prefix + L"uninstall.exe";
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snapshot == INVALID_HANDLE_VALUE) return false;
    PROCESSENTRY32W entry{sizeof entry};
    bool running = false;
    for (BOOL more = Process32FirstW(snapshot, &entry); more && !running; more = Process32NextW(snapshot, &entry)) {
        HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, entry.th32ProcessID);
        if (!process) continue;
        wchar_t path[1024] = {};
        DWORD size = 1024;
        if (QueryFullProcessImageNameW(process, 0, path, &size) && _wcsnicmp(path, prefix.c_str(), prefix.size()) == 0 &&
            _wcsicmp(path, uninstaller.c_str()) != 0)
            running = true;
        CloseHandle(process);
    }
    CloseHandle(snapshot);
    return running;
}

bool WriteResource(int id, std::wstring const& path) {
    HRSRC found = FindResourceW(nullptr, MAKEINTRESOURCEW(id), RT_RCDATA);
    if (!found) return false;
    HGLOBAL loaded = LoadResource(nullptr, found);
    void const* data = loaded ? LockResource(loaded) : nullptr;
    DWORD size = SizeofResource(nullptr, found);
    if (!data && size) return false;
    SHCreateDirectoryExW(nullptr, FolderOf(path).c_str(), nullptr);
    HANDLE file = CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) return false;
    DWORD written = 0;
    bool ok = WriteFile(file, data, size, &written, nullptr) && written == size;
    CloseHandle(file);
    return ok;
}

bool MakeShortcut(std::wstring const& target, std::wstring const& folder) {
    IShellLinkW* link = nullptr;
    if (FAILED(CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&link)))) return false;
    link->SetPath(target.c_str());
    link->SetWorkingDirectory(folder.c_str());
    link->SetDescription(Text().name);
    IPersistFile* file = nullptr;
    bool ok = SUCCEEDED(link->QueryInterface(IID_PPV_ARGS(&file))) && SUCCEEDED(file->Save(Shortcut().c_str(), TRUE));
    if (file) file->Release();
    link->Release();
    return ok;
}

void SetString(HKEY key, wchar_t const* name, std::wstring const& value) {
    RegSetValueExW(key, name, 0, REG_SZ, reinterpret_cast<BYTE const*>(value.c_str()), static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
}

void SetNumber(HKEY key, wchar_t const* name, DWORD value) {
    RegSetValueExW(key, name, 0, REG_DWORD, reinterpret_cast<BYTE const*>(&value), sizeof value);
}

/// The entry in Settings > Apps.
bool Register(std::wstring const& folder) {
    HKEY key = nullptr;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, UninstallKey().c_str(), 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr) != ERROR_SUCCESS)
        return false;
    std::wstring uninstall = L"\"" + folder + L"\\uninstall.exe\"";
    SetString(key, L"DisplayName", kDisplayName);
    SetString(key, L"DisplayVersion", kVersion);
    SetString(key, L"Publisher", kPublisher);
    SetString(key, L"DisplayIcon", folder + L"\\" + kExecutable);
    SetString(key, L"InstallLocation", folder);
    SetString(key, L"UninstallString", uninstall);
    SetString(key, L"QuietUninstallString", uninstall + L" /S");
    SetNumber(key, L"NoModify", 1);
    SetNumber(key, L"NoRepair", 1);
    SetNumber(key, L"EstimatedSize", kSizeKB);
    RegCloseKey(key);
    return true;
}

int Install() {
    if (!Ask(Text().install)) return 1;
    std::wstring folder = InstallFolder();
    // A version already there goes first, unless it runs.
    bool present = GetFileAttributesW(folder.c_str()) != INVALID_FILE_ATTRIBUTES;
    if (present && (Running(folder) || !RemoveTree(folder))) {
        Tell(Text().close_first, MB_ICONWARNING);
        return 1;
    }
    for (auto const& file : kFiles) {
        if (!WriteResource(file.id, folder + L"\\" + file.path)) {
            Tell(Text().failed, MB_ICONERROR);
            return 1;
        }
    }
    std::wstring executable = folder + L"\\" + kExecutable;
    if (!CopyFileW(ThisProgram().c_str(), (folder + L"\\uninstall.exe").c_str(), FALSE) || !Register(folder)) {
        Tell(Text().failed, MB_ICONERROR);
        return 1;
    }
    MakeShortcut(executable, folder);
    if (!silent && MessageBoxW(nullptr, Fill(Text().installed).c_str(), Title().c_str(), MB_YESNO | MB_ICONINFORMATION | MB_SETFOREGROUND) == IDYES)
        ShellExecuteW(nullptr, L"open", executable.c_str(), nullptr, folder.c_str(), SW_SHOWNORMAL);
    return 0;
}

int Uninstall() {
    std::wstring folder = InstallFolder();
    std::wstring self = ThisProgram();
    // A program cannot remove itself: the copy in the installed folder
    // hands over to a copy in the temporary folder.
    if (_wcsicmp(FolderOf(self).c_str(), folder.c_str()) == 0) {
        wchar_t temp[MAX_PATH] = {};
        GetTempPathW(MAX_PATH, temp);
        std::wstring copy = std::wstring(temp) + kKey + L"-uninstall.exe";
        if (!CopyFileW(self.c_str(), copy.c_str(), FALSE)) {
            Tell(Text().failed, MB_ICONERROR);
            return 1;
        }
        std::wstring arguments = silent ? L"/uninstall /S" : L"/uninstall";
        SHELLEXECUTEINFOW run{sizeof run};
        run.lpFile = copy.c_str();
        run.lpParameters = arguments.c_str();
        run.nShow = SW_SHOWNORMAL;
        return ShellExecuteExW(&run) ? 0 : 1;
    }
    if (!Ask(Text().uninstall)) return 1;
    // Nothing is removed while the application runs. The uninstaller goes
    // last, so a removal that stops halfway can be tried again.
    if (Running(folder) || !RemoveTree(folder, L"uninstall.exe")) {
        Tell(Text().close_first, MB_ICONWARNING);
        return 1;
    }
    // The copy in the folder that handed over may still be ending.
    bool removed = false;
    for (int attempt = 0; attempt < 20 && !removed; ++attempt) {
        removed = GetFileAttributesW(folder.c_str()) == INVALID_FILE_ATTRIBUTES || RemoveTree(folder);
        if (!removed) Sleep(250);
    }
    if (!removed) {
        Tell(Text().failed, MB_ICONERROR);
        return 1;
    }
    DeleteFileW(Shortcut().c_str());
    RegDeleteTreeW(HKEY_CURRENT_USER, UninstallKey().c_str());
    Tell(Text().uninstalled);
    return 0;
}

}  // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
    int count = 0;
    LPWSTR* arguments = CommandLineToArgvW(GetCommandLineW(), &count);
    bool uninstall = false;
    for (int i = 1; i < count; ++i) {
        if (_wcsicmp(arguments[i], L"/S") == 0) silent = true;
        if (_wcsicmp(arguments[i], L"/uninstall") == 0) uninstall = true;
    }
    LocalFree(arguments);
    // The copy in the installed folder removes, whatever its arguments.
    std::wstring self = ThisProgram();
    std::wstring name = self.substr(self.find_last_of(L'\\') + 1);
    if (_wcsicmp(name.c_str(), L"uninstall.exe") == 0) uninstall = true;
    CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    int code = uninstall ? Uninstall() : Install();
    CoUninitialize();
    return code;
}
