// Bern installer packer: the staged files, in the order of `setup_info.h`,
// as the one block the installer carries: their sizes, then all of them
// compressed together. It runs after signing, on what will be installed.
//
//   pack.exe <staged folder> <output>
//
// Generated build input; do not edit.

#include <windows.h>
#include <compressapi.h>

#include <cstdint>
#include <cstdio>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

#include "setup_info.h"

int wmain(int count, wchar_t** arguments) {
    if (count != 3) return 2;
    std::vector<uint64_t> sizes;
    std::vector<char> files;
    for (wchar_t const* file : kFiles) {
        std::ifstream in(std::wstring(arguments[1]) + L"\\" + file, std::ios::binary);
        if (!in) {
            std::fwprintf(stderr, L"bern: cannot read %ls\n", file);
            return 1;
        }
        size_t const start = files.size();
        files.insert(files.end(), std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
        sizes.push_back(files.size() - start);
    }

    COMPRESSOR_HANDLE compressor = nullptr;
    if (!CreateCompressor(COMPRESS_ALGORITHM_LZMS, nullptr, &compressor)) return 1;
    // Far smaller than with the default block: programs repeat themselves
    // across megabytes.
    DWORD block = 64 * 1024 * 1024;
    if (!SetCompressorInformation(compressor, COMPRESS_INFORMATION_CLASS_BLOCK_SIZE, &block, sizeof block)) return 1;
    SIZE_T room = 0;
    Compress(compressor, files.data(), files.size(), nullptr, 0, &room);
    std::vector<char> packed(room);
    SIZE_T size = 0;
    bool const done = Compress(compressor, files.data(), files.size(), packed.data(), packed.size(), &size);
    CloseCompressor(compressor);
    if (!done) {
        std::fwprintf(stderr, L"bern: cannot compress the files (%lu)\n", GetLastError());
        return 1;
    }

    std::ofstream out(arguments[2], std::ios::binary);
    out.write(reinterpret_cast<char const*>(sizes.data()), static_cast<std::streamsize>(sizes.size() * sizeof(uint64_t)));
    out.write(packed.data(), static_cast<std::streamsize>(size));
    return out.good() ? 0 : 1;
}
