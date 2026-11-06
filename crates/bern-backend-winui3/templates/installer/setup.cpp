// Bern installer. Puts the application in the user's own programs folder
// (%LOCALAPPDATA%\Programs), with a Start menu shortcut and an entry in
// Settings > Apps, and removes it again. The files it opens are registered
// for the user: "Open with", Default apps and the context menu of Explorer.
// No administrator rights; nothing is downloaded; the files travel inside
// this program, compressed by pack.cpp into one resource.
//
//   setup.exe              install, from a small window
//   setup.exe /S           install silently
//   uninstall.exe          remove, from the same window (a copy of this
//                          program in the installed folder)
//   uninstall.exe /S       remove silently
//
// Generated build input; do not edit.

#include <windows.h>
#include <commctrl.h>
#include <compressapi.h>
#include <d2d1.h>
#include <dwmapi.h>
#include <dwrite.h>
#include <knownfolders.h>
#include <objbase.h>
#include <shellapi.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <tlhelp32.h>
#include <wincodec.h>
#include <windowsx.h>
#include <wrl/client.h>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <cwchar>
#include <iterator>
#include <string>
#include <thread>
#include <vector>

#include "setup_info.h"

namespace {

using Microsoft::WRL::ComPtr;

constexpr int kPayload = 1001;

bool silent = false;
bool removing = false;

/// How far the work has come, from 0 to 1. The bar eases toward it.
std::atomic<float> reached{0};

enum class Outcome { Finished, CloseFirst, Failed };

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

/// `text` with `{name}` filled in.
std::wstring Fill(wchar_t const* text) {
    std::wstring out(text);
    std::wstring const key(L"{name}");
    std::wstring const name(Text().name);
    for (size_t at = out.find(key); at != std::wstring::npos; at = out.find(key, at + name.size()))
        out.replace(at, key.size(), name);
    return out;
}

std::wstring KnownFolder(KNOWNFOLDERID const& id) {
    PWSTR path = nullptr;
    std::wstring out;
    if (SUCCEEDED(SHGetKnownFolderPath(id, KF_FLAG_CREATE, nullptr, &path))) out = path;
    CoTaskMemFree(path);
    return out;
}

std::wstring Shortcut() { return KnownFolder(FOLDERID_Programs) + L"\\" + kShortcut + L".lnk"; }
std::wstring UninstallKey() { return std::wstring(L"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\") + kKey; }

/// The folder picked for a first installation, if one was.
std::wstring picked;

/// Where the application goes: what was picked, else where it already is,
/// else the user's programs folder.
std::wstring InstallFolder() {
    if (!picked.empty()) return picked;
    wchar_t folder[MAX_PATH] = {};
    DWORD size = sizeof folder;
    if (RegGetValueW(HKEY_CURRENT_USER, UninstallKey().c_str(), L"InstallLocation", RRF_RT_REG_SZ, nullptr, folder,
                     &size) == ERROR_SUCCESS)
        return folder;
    return KnownFolder(FOLDERID_UserProgramFiles) + L"\\" + kFolder;
}

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

/// The files this program carries, one after the other, and their sizes.
struct Unpacked {
    std::vector<uint64_t> sizes;
    std::vector<BYTE> bytes;
};

bool Unpack(Unpacked& files) {
    HRSRC found = FindResourceW(nullptr, MAKEINTRESOURCEW(kPayload), RT_RCDATA);
    HGLOBAL loaded = found ? LoadResource(nullptr, found) : nullptr;
    auto const* data = static_cast<BYTE const*>(loaded ? LockResource(loaded) : nullptr);
    size_t const size = found ? SizeofResource(nullptr, found) : 0;
    size_t const head = std::size(kFiles) * sizeof(uint64_t);
    if (!data || size < head) return false;
    files.sizes.resize(std::size(kFiles));
    std::memcpy(files.sizes.data(), data, head);
    uint64_t total = 0;
    for (uint64_t file : files.sizes) total += file;
    files.bytes.resize(total);
    DECOMPRESSOR_HANDLE decompressor = nullptr;
    if (!CreateDecompressor(COMPRESS_ALGORITHM_LZMS, nullptr, &decompressor)) return false;
    SIZE_T written = 0;
    bool ok = Decompress(decompressor, data + head, size - head, files.bytes.data(), files.bytes.size(), &written) && written == total;
    CloseDecompressor(decompressor);
    return ok;
}

bool WriteBytes(BYTE const* data, uint64_t size, std::wstring const& path) {
    SHCreateDirectoryExW(nullptr, FolderOf(path).c_str(), nullptr);
    HANDLE file = CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) return false;
    DWORD written = 0;
    bool ok = WriteFile(file, data, static_cast<DWORD>(size), &written, nullptr) && written == size;
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

std::wstring Classes(std::wstring const& key) { return L"Software\\Classes\\" + key; }
/// What Windows knows the application's files by.
std::wstring ProgId() { return std::wstring(kKey) + L".file"; }
std::wstring OwnKey() { return std::wstring(L"Software\\") + kKey; }
std::wstring Capabilities() { return OwnKey() + L"\\Capabilities"; }

/// Sets `name` of the key at `path`, or its default value without a name.
void SetValue(std::wstring const& path, wchar_t const* name, std::wstring const& value) {
    HKEY key = nullptr;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, path.c_str(), 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr) != ERROR_SUCCESS)
        return;
    SetString(key, name, value);
    RegCloseKey(key);
}

/// Removes the application's context menu items from the verbs at `shell`.
void RemoveMenuItems(std::wstring const& shell) {
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, shell.c_str(), 0, KEY_READ, &key) != ERROR_SUCCESS) return;
    std::wstring const prefix = std::wstring(kKey) + L".";
    std::vector<std::wstring> items;
    wchar_t name[256];
    for (DWORD index = 0;; ++index) {
        DWORD size = static_cast<DWORD>(std::size(name));
        if (RegEnumKeyExW(key, index, name, &size, nullptr, nullptr, nullptr, nullptr) != ERROR_SUCCESS) break;
        if (std::wcsncmp(name, prefix.c_str(), prefix.size()) == 0) items.push_back(name);
    }
    RegCloseKey(key);
    for (auto const& item : items) RegDeleteTreeW(HKEY_CURRENT_USER, (shell + L"\\" + item).c_str());
}

/// Takes back what was registered for the application's files, going by
/// what the registry says, so an update drops what it no longer opens. What
/// the user chose as default stays theirs: Windows falls back by itself.
void ForgetFileTypes() {
    HKEY key = nullptr;
    auto associations = Capabilities() + L"\\FileAssociations";
    if (RegOpenKeyExW(HKEY_CURRENT_USER, associations.c_str(), 0, KEY_READ, &key) == ERROR_SUCCESS) {
        wchar_t name[256];
        for (DWORD index = 0;; ++index) {
            DWORD size = static_cast<DWORD>(std::size(name));
            if (RegEnumValueW(key, index, name, &size, nullptr, nullptr, nullptr, nullptr) != ERROR_SUCCESS) break;
            std::wstring type = name;
            RegDeleteKeyValueW(HKEY_CURRENT_USER, Classes(type + L"\\OpenWithProgids").c_str(), ProgId().c_str());
            RemoveMenuItems(Classes(L"SystemFileAssociations\\" + type + L"\\shell"));
        }
        RegCloseKey(key);
    }
    RemoveMenuItems(Classes(L"Directory\\shell"));
    RemoveMenuItems(Classes(L"*\\shell"));
    RegDeleteTreeW(HKEY_CURRENT_USER, Classes(ProgId()).c_str());
    RegDeleteTreeW(HKEY_CURRENT_USER, Classes(std::wstring(L"Applications\\") + kExecutable).c_str());
    RegDeleteTreeW(HKEY_CURRENT_USER, Capabilities().c_str());
    RegDeleteKeyW(HKEY_CURRENT_USER, OwnKey().c_str());
    RegDeleteKeyValueW(HKEY_CURRENT_USER, L"Software\\RegisteredApplications", kKey);
}

/// A context menu item among the verbs at `shell`. Several selected files
/// each start the program, and the running instance takes them all.
void AddMenuItem(std::wstring const& shell, int menu, std::wstring const& program, std::wstring const& command) {
    auto item = shell + L"\\" + kKey + L"." + std::to_wstring(menu);
    SetValue(item, L"MUIVerb", Text().menus[menu]);
    SetValue(item, L"Icon", program);
    SetValue(item, L"MultiSelectModel", L"Player");
    SetValue(item + L"\\command", nullptr, command);
}

/// Registers the application's files for the user: a file type with
/// "Open with", the application in Default apps, where the user can make it
/// the default for each extension, and the context menu items.
void RegisterFileTypes(std::wstring const& folder) {
    ForgetFileTypes();
    if (kFileTypes[0].type) {
        std::wstring const program = folder + L"\\" + kExecutable;
        std::wstring const command = L"\"" + program + L"\" \"%1\"";
        auto progId = Classes(ProgId());
        SetValue(progId, nullptr, kDisplayName);
        SetValue(progId + L"\\DefaultIcon", nullptr, program + L",0");
        SetValue(progId + L"\\shell\\open", L"FriendlyAppName", kDisplayName);
        SetValue(progId + L"\\shell\\open\\command", nullptr, command);
        auto application = Classes(std::wstring(L"Applications\\") + kExecutable);
        SetValue(application, L"FriendlyAppName", kDisplayName);
        SetValue(application + L"\\shell\\open\\command", nullptr, command);
        SetValue(Capabilities(), L"ApplicationName", kDisplayName);
        SetValue(Capabilities(), L"ApplicationDescription", kDisplayName);
        SetValue(L"Software\\RegisteredApplications", kKey, Capabilities());
        for (auto const* file = kFileTypes; file->type; ++file) {
            std::wstring const type = file->type;
            bool const extension = type[0] == L'.';
            if (extension) {
                HKEY key = nullptr;
                auto list = Classes(type + L"\\OpenWithProgids");
                if (RegCreateKeyExW(HKEY_CURRENT_USER, list.c_str(), 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr) == ERROR_SUCCESS) {
                    RegSetValueExW(key, ProgId().c_str(), 0, REG_NONE, nullptr, 0);
                    RegCloseKey(key);
                }
                SetValue(application + L"\\SupportedTypes", type.c_str(), L"");
                SetValue(Capabilities() + L"\\FileAssociations", type.c_str(), ProgId());
            }
            if (file->menu < 0) continue;
            std::wstring shell = extension ? L"SystemFileAssociations\\" + type + L"\\shell"
                                           : std::wstring(type == L"folder" ? L"Directory\\shell" : L"*\\shell");
            AddMenuItem(Classes(shell), file->menu, program, command);
        }
    }
    SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST | SHCNF_FLUSH, nullptr, nullptr);
}

/// The version in Settings > Apps, if one is installed.
std::wstring InstalledVersion() {
    wchar_t version[64] = {};
    DWORD size = sizeof version;
    if (RegGetValueW(HKEY_CURRENT_USER, UninstallKey().c_str(), L"DisplayVersion", RRF_RT_REG_SZ, nullptr, version, &size) != ERROR_SUCCESS)
        return {};
    return version;
}

Outcome Install() {
    std::wstring folder = InstallFolder();
    bool present = GetFileAttributesW(folder.c_str()) != INVALID_FILE_ATTRIBUTES;
    if (present && Running(folder)) return Outcome::CloseFirst;
    reached = 0.1f;
    Unpacked files;
    if (!Unpack(files)) return Outcome::Failed;
    reached = 0.6f;
    // A version already there goes only now that the new one is at hand.
    // A folder that was already there and is not ours stays as it is.
    if (present && !InstalledVersion().empty() && !RemoveTree(folder)) return Outcome::CloseFirst;
    BYTE const* next = files.bytes.data();
    for (size_t i = 0; i < std::size(kFiles); ++i) {
        if (!WriteBytes(next, files.sizes[i], folder + L"\\" + kFiles[i])) return Outcome::Failed;
        next += files.sizes[i];
        reached = 0.6f + 0.35f * static_cast<float>(next - files.bytes.data()) / static_cast<float>(files.bytes.size());
    }
    if (!CopyFileW(ThisProgram().c_str(), (folder + L"\\uninstall.exe").c_str(), FALSE) || !Register(folder)) return Outcome::Failed;
    MakeShortcut(folder + L"\\" + kExecutable, folder);
    RegisterFileTypes(folder);
    reached = 1;
    return Outcome::Finished;
}

Outcome Uninstall() {
    std::wstring folder = InstallFolder();
    // Nothing is removed while the application runs. The uninstaller goes
    // last, so a removal that stops halfway can be tried again.
    if (Running(folder) || !RemoveTree(folder, L"uninstall.exe")) return Outcome::CloseFirst;
    reached = 0.7f;
    // The copy in the folder that handed over may still be ending.
    bool removed = false;
    for (int attempt = 0; attempt < 20 && !removed; ++attempt) {
        removed = GetFileAttributesW(folder.c_str()) == INVALID_FILE_ATTRIBUTES || RemoveTree(folder);
        if (!removed) Sleep(250);
    }
    if (!removed) return Outcome::Failed;
    DeleteFileW(Shortcut().c_str());
    ForgetFileTypes();
    SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST | SHCNF_FLUSH, nullptr, nullptr);
    RegDeleteTreeW(HKEY_CURRENT_USER, UninstallKey().c_str());
    // “Open at Login” of the application's status item, and Task Manager's
    // note on it.
    RegDeleteKeyValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run", kKey);
    RegDeleteKeyValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run", kKey);
    reached = 1;
    return Outcome::Finished;
}

/// A program cannot remove itself: the copy in the installed folder hands
/// over to a copy in the temporary folder. True when it has.
bool HandOver(int& code) {
    std::wstring self = ThisProgram();
    if (_wcsicmp(FolderOf(self).c_str(), InstallFolder().c_str()) != 0) return false;
    wchar_t temp[MAX_PATH] = {};
    GetTempPathW(MAX_PATH, temp);
    std::wstring copy = std::wstring(temp) + kKey + L"-uninstall.exe";
    code = 1;
    if (!CopyFileW(self.c_str(), copy.c_str(), FALSE)) return true;
    std::wstring arguments = silent ? L"/uninstall /S" : L"/uninstall";
    SHELLEXECUTEINFOW run{sizeof run};
    run.lpFile = copy.c_str();
    run.lpParameters = arguments.c_str();
    run.nShow = SW_SHOWNORMAL;
    if (ShellExecuteExW(&run)) code = 0;
    return true;
}

// The window: one quiet surface drawn with Direct2D, and one real button,
// which brings the keyboard and screen readers with it. All sizes are in
// device-independent pixels.

constexpr float kWidth = 400;
constexpr float kHeight = 340;
constexpr float kPicture = 96;
constexpr float kPictureTop = 50;
constexpr float kNameTop = 162;
constexpr float kLineTop = 194;
constexpr float kPlaceTop = 218;
constexpr float kPlaceHeight = 22;
constexpr float kPlaceInset = 28;
constexpr float kActionTop = 250;
constexpr float kActionWidth = 168;
constexpr float kActionHeight = 38;
constexpr float kBarHeight = 3;
/// Around the button, for the ring that shows the keyboard is on it.
constexpr float kRing = 4;
constexpr float kCloseSize = 32;
constexpr float kCloseInset = 10;

constexpr UINT kFinished = WM_APP;
constexpr int kPlace = 100;
constexpr UINT_PTR kTick = 1;

enum class Phase { Ready, Working, Done, Failed };

struct Colors {
    D2D1_COLOR_F surface, text, soft, faint, action, hover, pressed, on_action;
};

HWND window = nullptr;
HWND button = nullptr;
/// The folder of a first installation: pressing it picks another.
HWND place = nullptr;
UINT dpi = 96;
bool dark = false;
Phase phase = Phase::Ready;
Outcome outcome = Outcome::Failed;
bool ended = false;
int code = 1;
std::wstring line;
float shown = 0;
bool over_action = false;
bool over_close = false;

ComPtr<ID2D1Factory> d2d;
ComPtr<IDWriteFactory> dwrite;
ComPtr<ID2D1DCRenderTarget> target;
ComPtr<ID2D1Bitmap> picture;
ComPtr<IDWriteTextFormat> name_format;
ComPtr<IDWriteTextFormat> line_format;
ComPtr<IDWriteTextFormat> action_format;
ComPtr<IDWriteTextFormat> place_format;
ComPtr<IDWriteTextFormat> pencil_format;

int Px(float dip) { return static_cast<int>(dip * static_cast<float>(dpi) / 96.0f + 0.5f); }
float Dip(LONG px) { return static_cast<float>(px) * 96.0f / static_cast<float>(dpi); }

Colors Palette() {
    using D2D1::ColorF;
    if (dark)
        return {ColorF(0x202023), ColorF(0xF5F5F7), ColorF(0xF5F5F7, 0.58f), ColorF(0xF5F5F7, 0.14f),
                ColorF(0xF5F5F7), ColorF(0xFFFFFF), ColorF(0xC8C8CD), ColorF(0x1C1C1F)};
    return {ColorF(0xFDFDFD), ColorF(0x1C1C1F), ColorF(0x1C1C1F, 0.56f), ColorF(0x1C1C1F, 0.09f),
            ColorF(0x1C1C1F), ColorF(0x3A3A40), ColorF(0x000000), ColorF(0xFFFFFF)};
}

bool SystemIsDark() {
    DWORD light = 1;
    DWORD size = sizeof light;
    RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", L"AppsUseLightTheme", RRF_RT_REG_DWORD,
                 nullptr, &light, &size);
    return light == 0;
}

ComPtr<ID2D1SolidColorBrush> Brush(D2D1_COLOR_F const& color) {
    ComPtr<ID2D1SolidColorBrush> brush;
    target->CreateSolidColorBrush(color, brush.GetAddressOf());
    return brush;
}

D2D1_RECT_F CloseArea() {
    return {kWidth - kCloseInset - kCloseSize, kCloseInset, kWidth - kCloseInset, kCloseInset + kCloseSize};
}

bool OnClose(POINT point) {
    D2D1_RECT_F area = CloseArea();
    float x = Dip(point.x), y = Dip(point.y);
    return phase != Phase::Working && x >= area.left && x < area.right && y >= area.top && y < area.bottom;
}

ComPtr<IDWriteTextFormat> Format(wchar_t const* family, DWRITE_FONT_WEIGHT weight, float size) {
    ComPtr<IDWriteTextFormat> format;
    dwrite->CreateTextFormat(family, nullptr, weight, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, Text().language, format.GetAddressOf());
    format->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
    return format;
}

bool HasFont(wchar_t const* family) {
    ComPtr<IDWriteFontCollection> fonts;
    UINT32 index = 0;
    BOOL exists = FALSE;
    return SUCCEEDED(dwrite->GetSystemFontCollection(fonts.GetAddressOf())) && SUCCEEDED(fonts->FindFamilyName(family, &index, &exists)) && exists;
}

bool StartGraphics() {
    if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, d2d.GetAddressOf()))) return false;
    if (FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory), reinterpret_cast<IUnknown**>(dwrite.GetAddressOf())))) return false;
    auto properties = D2D1::RenderTargetProperties(D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE));
    if (FAILED(d2d->CreateDCRenderTarget(&properties, target.GetAddressOf()))) return false;
    // Soft text in color fringes looks dirty on a surface this quiet.
    target->SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
    bool variable = HasFont(L"Segoe UI Variable Text");
    wchar_t const* display = variable ? L"Segoe UI Variable Display" : L"Segoe UI";
    wchar_t const* text = variable ? L"Segoe UI Variable Text" : L"Segoe UI";
    name_format = Format(display, DWRITE_FONT_WEIGHT_SEMI_BOLD, 22);
    line_format = Format(text, DWRITE_FONT_WEIGHT_NORMAL, 13);
    action_format = Format(text, DWRITE_FONT_WEIGHT_MEDIUM, 14);
    action_format->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
    place_format = Format(text, DWRITE_FONT_WEIGHT_NORMAL, 12);
    place_format->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
    // Windows 10 and 11 both have this font and its pencil.
    pencil_format = Format(L"Segoe MDL2 Assets", DWRITE_FONT_WEIGHT_NORMAL, 13);
    pencil_format->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
    place_format->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
    ComPtr<IDWriteInlineObject> dots;
    dwrite->CreateEllipsisTrimmingSign(place_format.Get(), dots.GetAddressOf());
    DWRITE_TRIMMING place_trimming{DWRITE_TRIMMING_GRANULARITY_CHARACTER, 0, 0};
    place_format->SetTrimming(&place_trimming, dots.Get());
    // A long name ends in an ellipsis rather than pushing the rest down.
    name_format->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
    ComPtr<IDWriteInlineObject> ellipsis;
    dwrite->CreateEllipsisTrimmingSign(name_format.Get(), ellipsis.GetAddressOf());
    DWRITE_TRIMMING trimming{DWRITE_TRIMMING_GRANULARITY_CHARACTER, 0, 0};
    name_format->SetTrimming(&trimming, ellipsis.Get());
    return true;
}

/// The application's icon, at the size it is drawn on this screen.
void LoadPicture() {
    picture.Reset();
    HICON icon = nullptr;
    int size = Px(kPicture);
    if (FAILED(LoadIconWithScaleDown(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(1), size, size, &icon))) return;
    ComPtr<IWICImagingFactory> images;
    ComPtr<IWICBitmap> bitmap;
    ComPtr<IWICFormatConverter> converter;
    if (SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(images.GetAddressOf()))) &&
        SUCCEEDED(images->CreateBitmapFromHICON(icon, bitmap.GetAddressOf())) && SUCCEEDED(images->CreateFormatConverter(converter.GetAddressOf())) &&
        SUCCEEDED(converter->Initialize(bitmap.Get(), GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, nullptr, 0, WICBitmapPaletteTypeMedianCut)))
        target->CreateBitmapFromWicBitmap(converter.Get(), picture.GetAddressOf());
    DestroyIcon(icon);
}

void PlaceButton() {
    if (place) {
        SetWindowPos(place, nullptr, Px(kPlaceInset), Px(kPlaceTop), Px(kWidth - 2 * kPlaceInset), Px(kPlaceHeight),
                     SWP_NOZORDER | SWP_NOACTIVATE);
    }
    float left = (kWidth - kActionWidth) / 2 - kRing;
    SetWindowPos(button, nullptr, Px(left), Px(kActionTop - kRing), Px(kActionWidth + 2 * kRing), Px(kActionHeight + 2 * kRing), SWP_NOZORDER | SWP_NOACTIVATE);
}

/// What suits this screen's density.
void Scale() {
    target->SetDpi(static_cast<float>(dpi), static_cast<float>(dpi));
    LoadPicture();
    PlaceButton();
}

void ApplyTheme() {
    dark = SystemIsDark();
    BOOL value = dark;
    DwmSetWindowAttribute(window, DWMWA_USE_IMMERSIVE_DARK_MODE, &value, sizeof value);
    InvalidateRect(window, nullptr, FALSE);
    InvalidateRect(button, nullptr, FALSE);
    if (place) InvalidateRect(place, nullptr, FALSE);
}

void Paint(HDC dc) {
    RECT area;
    GetClientRect(window, &area);
    Colors const colors = Palette();
    target->BindDC(dc, &area);
    target->BeginDraw();
    target->Clear(colors.surface);

    if (picture) {
        float left = (kWidth - kPicture) / 2;
        target->DrawBitmap(picture.Get(), {left, kPictureTop, left + kPicture, kPictureTop + kPicture}, 1, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    }
    std::wstring name(Text().name);
    float inset = 28;
    target->DrawText(name.c_str(), static_cast<UINT32>(name.size()), name_format.Get(), {inset, kNameTop, kWidth - inset, kNameTop + 30}, Brush(colors.text).Get());
    target->DrawText(line.c_str(), static_cast<UINT32>(line.size()), line_format.Get(), {inset, kLineTop, kWidth - inset, kActionTop - 4}, Brush(colors.soft).Get());

    if (phase == Phase::Working) {
        float left = (kWidth - kActionWidth) / 2;
        float top = kActionTop + (kActionHeight - kBarHeight) / 2;
        float radius = kBarHeight / 2;
        target->FillRoundedRectangle({{left, top, left + kActionWidth, top + kBarHeight}, radius, radius}, Brush(colors.faint).Get());
        float width = kActionWidth * shown;
        if (width >= kBarHeight)
            target->FillRoundedRectangle({{left, top, left + width, top + kBarHeight}, radius, radius}, Brush(colors.text).Get());
    } else {
        D2D1_RECT_F close = CloseArea();
        if (over_close) target->FillRoundedRectangle({close, 6, 6}, Brush(colors.faint).Get());
        float x = (close.left + close.right) / 2, y = (close.top + close.bottom) / 2, arm = 4.5f;
        auto mark = Brush(colors.soft);
        target->DrawLine({x - arm, y - arm}, {x + arm, y + arm}, mark.Get(), 1);
        target->DrawLine({x - arm, y + arm}, {x + arm, y - arm}, mark.Get(), 1);
    }
    target->EndDraw();
}

void DrawPlace(DRAWITEMSTRUCT const& item) {
    Colors const colors = Palette();
    target->BindDC(item.hDC, &item.rcItem);
    target->BeginDraw();
    target->Clear(colors.surface);
    float width = Dip(item.rcItem.right - item.rcItem.left);
    float height = Dip(item.rcItem.bottom - item.rcItem.top);
    bool focused = (item.itemState & ODS_FOCUS) && !(item.itemState & ODS_NOFOCUSRECT);
    wchar_t label[MAX_PATH] = {};
    UINT32 length = static_cast<UINT32>(GetWindowTextW(item.hwndItem, label, MAX_PATH));
    // The folder, as much of it as fits, and the pencil that says it can change.
    float mark = 16, gap = 8;
    float room = width - mark - gap;
    ComPtr<IDWriteTextLayout> folder;
    dwrite->CreateTextLayout(label, length, place_format.Get(), room, height, folder.GetAddressOf());
    folder->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
    DWRITE_TEXT_METRICS size{};
    folder->GetMetrics(&size);
    float shown = size.width < room ? size.width : room;
    float left = (width - shown - gap - mark) / 2;
    target->DrawTextLayout({left, 0}, folder.Get(), Brush(focused ? colors.text : colors.soft).Get());
    float start = left + shown + gap;
    target->DrawText(L"\uE70F", 1, pencil_format.Get(), {start, 0, start + mark, height}, Brush(colors.text).Get());
    target->EndDraw();
}

LRESULT CALLBACK PlaceProc(HWND self, UINT message, WPARAM wide, LPARAM wide_too, UINT_PTR, DWORD_PTR) {
    if (message == WM_SETCURSOR) {
        SetCursor(LoadCursorW(nullptr, IDC_HAND));
        return TRUE;
    }
    return DefSubclassProc(self, message, wide, wide_too);
}

/// Ask for the folder of a first installation; the application gets a
/// folder of its own inside it.
void PickFolder() {
    ComPtr<IFileOpenDialog> dialog;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&dialog)))) return;
    DWORD options = 0;
    dialog->GetOptions(&options);
    dialog->SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
    if (FAILED(dialog->Show(window))) return;
    ComPtr<IShellItem> item;
    PWSTR path = nullptr;
    if (FAILED(dialog->GetResult(item.GetAddressOf())) || FAILED(item->GetDisplayName(SIGDN_FILESYSPATH, &path))) return;
    std::wstring folder(path);
    CoTaskMemFree(path);
    if (!folder.empty() && folder.back() == L'\\') folder.pop_back();
    picked = folder + L"\\" + kFolder;
    SetWindowTextW(place, picked.c_str());
    InvalidateRect(place, nullptr, FALSE);
}

void DrawAction(DRAWITEMSTRUCT const& item) {
    Colors const colors = Palette();
    target->BindDC(item.hDC, &item.rcItem);
    target->BeginDraw();
    target->Clear(colors.surface);
    float width = Dip(item.rcItem.right - item.rcItem.left);
    float height = Dip(item.rcItem.bottom - item.rcItem.top);
    D2D1_RECT_F face{kRing, kRing, width - kRing, height - kRing};
    float radius = 9;
    bool pressed = item.itemState & ODS_SELECTED;
    target->FillRoundedRectangle({face, radius, radius}, Brush(pressed ? colors.pressed : over_action ? colors.hover : colors.action).Get());
    if ((item.itemState & ODS_FOCUS) && !(item.itemState & ODS_NOFOCUSRECT)) {
        D2D1_RECT_F ring{1.5f, 1.5f, width - 1.5f, height - 1.5f};
        target->DrawRoundedRectangle({ring, radius + 2.5f, radius + 2.5f}, Brush(colors.text).Get(), 1.5f);
    }
    wchar_t label[64] = {};
    int length = GetWindowTextW(item.hwndItem, label, 64);
    target->DrawText(label, static_cast<UINT32>(length), action_format.Get(), face, Brush(colors.on_action).Get());
    target->EndDraw();
}

void Start() {
    phase = Phase::Working;
    ended = false;
    shown = 0;
    reached = 0;
    line = removing ? Text().uninstalling : Text().installing;
    ShowWindow(button, SW_HIDE);
    if (place) ShowWindow(place, SW_HIDE);
    SetFocus(window);
    SetTimer(window, kTick, 15, nullptr);
    InvalidateRect(window, nullptr, FALSE);
    std::thread([] {
        CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
        Outcome result = removing ? Uninstall() : Install();
        CoUninitialize();
        PostMessageW(window, kFinished, static_cast<WPARAM>(result), 0);
    }).detach();
}

/// The work is over and the bar has shown it.
void Settle() {
    KillTimer(window, kTick);
    bool fine = outcome == Outcome::Finished;
    phase = fine ? Phase::Done : Phase::Failed;
    code = fine ? 0 : 1;
    if (fine) {
        line = removing ? Text().uninstalled : Text().installed;
        SetWindowTextW(button, removing ? Text().done : Text().open);
    } else {
        line = Fill(outcome == Outcome::CloseFirst ? Text().close_first : Text().failed);
        SetWindowTextW(button, Text().retry);
    }
    // A folder that could not be written to can be changed for another.
    if (place && !fine) ShowWindow(place, SW_SHOW);
    ShowWindow(button, SW_SHOW);
    SetFocus(button);
    InvalidateRect(window, nullptr, FALSE);
}

void Act() {
    if (phase == Phase::Ready || phase == Phase::Failed) {
        Start();
    } else if (phase == Phase::Done) {
        if (!removing) {
            std::wstring folder = InstallFolder();
            ShellExecuteW(nullptr, L"open", (folder + L"\\" + kExecutable).c_str(), nullptr, folder.c_str(), SW_SHOWNORMAL);
        }
        DestroyWindow(window);
    }
}

LRESULT CALLBACK ButtonProc(HWND self, UINT message, WPARAM wide, LPARAM wide_too, UINT_PTR, DWORD_PTR) {
    if (message == WM_MOUSEMOVE && !over_action) {
        over_action = true;
        TRACKMOUSEEVENT track{sizeof track, TME_LEAVE, self, 0};
        TrackMouseEvent(&track);
        InvalidateRect(self, nullptr, FALSE);
    } else if (message == WM_MOUSELEAVE) {
        over_action = false;
        InvalidateRect(self, nullptr, FALSE);
    }
    return DefSubclassProc(self, message, wide, wide_too);
}

LRESULT CALLBACK WindowProc(HWND self, UINT message, WPARAM wide, LPARAM wide_too) {
    switch (message) {
    // The whole window is the surface: no title bar, the system's corners
    // and shadow, and a move from anywhere but the controls.
    case WM_NCCALCSIZE:
        if (wide) return 0;
        break;
    case WM_NCACTIVATE:
        return DefWindowProcW(self, message, wide, -1);
    case WM_NCHITTEST: {
        POINT point{GET_X_LPARAM(wide_too), GET_Y_LPARAM(wide_too)};
        ScreenToClient(self, &point);
        return OnClose(point) ? HTCLIENT : HTCAPTION;
    }
    case WM_MOUSEMOVE:
        if (!over_close) {
            over_close = true;
            TRACKMOUSEEVENT track{sizeof track, TME_LEAVE, self, 0};
            TrackMouseEvent(&track);
            InvalidateRect(self, nullptr, FALSE);
        }
        return 0;
    case WM_MOUSELEAVE:
        over_close = false;
        InvalidateRect(self, nullptr, FALSE);
        return 0;
    case WM_LBUTTONUP:
        PostMessageW(self, WM_CLOSE, 0, 0);
        return 0;
    case WM_ERASEBKGND:
        return 1;
    case WM_PAINT: {
        PAINTSTRUCT paint;
        HDC dc = BeginPaint(self, &paint);
        Paint(dc);
        EndPaint(self, &paint);
        return 0;
    }
    case WM_DRAWITEM: {
        auto const& item = *reinterpret_cast<DRAWITEMSTRUCT const*>(wide_too);
        if (item.CtlID == kPlace) DrawPlace(item);
        else DrawAction(item);
        return TRUE;
    }
    case WM_COMMAND:
        if (LOWORD(wide) == IDOK) Act();
        if (LOWORD(wide) == kPlace) PickFolder();
        if (LOWORD(wide) == IDCANCEL) PostMessageW(self, WM_CLOSE, 0, 0);
        return 0;
    case WM_TIMER:
        shown += (reached.load() - shown) * 0.14f;
        if (ended && shown > 0.995f) Settle();
        else InvalidateRect(self, nullptr, FALSE);
        return 0;
    case kFinished:
        outcome = static_cast<Outcome>(wide);
        ended = true;
        if (outcome != Outcome::Finished) Settle();
        return 0;
    case WM_DPICHANGED: {
        dpi = HIWORD(wide);
        auto const* area = reinterpret_cast<RECT const*>(wide_too);
        SetWindowPos(self, nullptr, area->left, area->top, area->right - area->left, area->bottom - area->top, SWP_NOZORDER | SWP_NOACTIVATE);
        Scale();
        return 0;
    }
    case WM_SETTINGCHANGE:
        if (wide_too && std::wcscmp(reinterpret_cast<wchar_t const*>(wide_too), L"ImmersiveColorSet") == 0) ApplyTheme();
        break;
    case WM_CLOSE:
        // Half an installation helps nobody.
        if (phase != Phase::Working) DestroyWindow(self);
        return 0;
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcW(self, message, wide, wide_too);
}

int ShowWindowAndWait() {
    if (!StartGraphics()) return 1;
    HINSTANCE instance = GetModuleHandleW(nullptr);
    WNDCLASSEXW kind{sizeof kind};
    kind.lpfnWndProc = WindowProc;
    kind.hInstance = instance;
    kind.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    kind.hIcon = LoadIconW(instance, MAKEINTRESOURCEW(1));
    kind.lpszClassName = L"BernSetup";
    RegisterClassExW(&kind);
    std::wstring title = Fill(Text().title);
    window = CreateWindowExW(0, kind.lpszClassName, title.c_str(), WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
                             CW_USEDEFAULT, CW_USEDEFAULT, 400, 340, nullptr, nullptr, instance, nullptr);
    if (!window) return 1;
    dpi = GetDpiForWindow(window);

    std::wstring installed = removing ? L"" : InstalledVersion();
    bool update = !installed.empty() && installed != kVersion;
    line = update ? installed + L"  →  " + kVersion : kVersion;
    wchar_t const* action = removing ? Text().uninstall : update ? Text().update : installed.empty() ? Text().install : Text().reinstall;
    button = CreateWindowExW(0, L"BUTTON", action, WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW, 0, 0, 0, 0, window,
                             reinterpret_cast<HMENU>(static_cast<INT_PTR>(IDOK)), instance, nullptr);
    SetWindowSubclass(button, ButtonProc, 0, 0);
    if (!removing && installed.empty()) {
        place = CreateWindowExW(0, L"BUTTON", InstallFolder().c_str(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW,
                                0, 0, 0, 0, window, reinterpret_cast<HMENU>(static_cast<INT_PTR>(kPlace)), instance, nullptr);
        SetWindowSubclass(place, PlaceProc, 0, 0);
    }
    Scale();
    ApplyTheme();
    DWM_WINDOW_CORNER_PREFERENCE round = DWMWCP_ROUND;
    DwmSetWindowAttribute(window, DWMWA_WINDOW_CORNER_PREFERENCE, &round, sizeof round);
    // The keyboard's ring shows only once the keyboard is used.
    SendMessageW(window, WM_CHANGEUISTATE, MAKEWPARAM(UIS_SET, UISF_HIDEFOCUS), 0);

    MONITORINFO monitor{sizeof monitor};
    GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTOPRIMARY), &monitor);
    RECT const& work = monitor.rcWork;
    int width = Px(kWidth), height = Px(kHeight);
    SetWindowPos(window, nullptr, work.left + (work.right - work.left - width) / 2, work.top + (work.bottom - work.top - height) / 2, width, height,
                 SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    ShowWindow(window, SW_SHOW);
    SetForegroundWindow(window);
    SetFocus(button);

    MSG message;
    while (GetMessageW(&message, nullptr, 0, 0) > 0) {
        if (IsDialogMessageW(window, &message)) continue;
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    return code;
}

}  // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
    int count = 0;
    LPWSTR* arguments = CommandLineToArgvW(GetCommandLineW(), &count);
    for (int i = 1; i < count; ++i) {
        if (_wcsicmp(arguments[i], L"/S") == 0) silent = true;
        if (_wcsicmp(arguments[i], L"/uninstall") == 0) removing = true;
    }
    LocalFree(arguments);
    // The copy in the installed folder removes, whatever its arguments.
    std::wstring self = ThisProgram();
    std::wstring name = self.substr(self.find_last_of(L'\\') + 1);
    if (_wcsicmp(name.c_str(), L"uninstall.exe") == 0) removing = true;
    int result = 0;
    if (removing && HandOver(result)) return result;
    CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (silent) result = (removing ? Uninstall() : Install()) == Outcome::Finished ? 0 : 1;
    else result = ShowWindowAndWait();
    CoUninitialize();
    return result;
}
