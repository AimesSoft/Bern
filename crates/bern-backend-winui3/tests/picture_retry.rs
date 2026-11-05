//! Pictures from the network come after a failure, and an address that will
//! not give one is not asked again. Exercises the actual `bern_pictures.h`
//! against a local server that fails on purpose, with the waits shortened.
//! Compiling it needs Windows and the Visual Studio C++ tools.

#[cfg(windows)]
#[path = "../../bern-target-api/tests/faulty_server/mod.rs"]
mod faulty_server;

#[cfg(windows)]
const TESTS: &str = r#"
#include <windows.h>
#include <cstdio>
#include <cstdlib>
#include <thread>
#include "bern_pictures.h"

#pragma comment(lib, "windowsapp.lib")

using namespace bern::pictures;
using namespace std::chrono_literals;
using Load = winrt::Windows::Foundation::IAsyncOperation<Bytes>;

static std::string base;

#define EXPECT(holds) \
    do { if (!(holds)) { std::printf("line %d: %s\n", __LINE__, #holds); std::fflush(stdout); std::exit(1); } } while (false)

/// The test's own questions, which count for nothing.
static std::string Text(std::string const& path) {
    http::HttpClient client;
    return winrt::to_string(client.GetStringAsync(foundation::Uri(winrt::to_hstring(base + path))).get());
}

static int Asked(std::string const& path) { return std::atoi(Text("/asked" + path).c_str()); }

/// How often `path` was asked for, once that has stopped changing.
static int Settled(std::string const& path) {
    int count = Asked(path), still = 0;
    while (still < 4 || count == 0) {
        std::this_thread::sleep_for(250ms);
        int now = Asked(path);
        still = now == count ? still + 1 : 0;
        count = now;
    }
    return count;
}

static bool Picture(Bytes const& bytes) { return bytes.Length() > 0 && bytes.data()[0] == 0x89; }

static Load Get(std::string const& url, std::shared_ptr<Waiter> waiter = std::make_shared<Waiter>()) {
    return Until(url, std::move(waiter), Picture);
}

int main(int, char** arguments) {
    winrt::init_apartment();
    base = arguments[1];
    std::string silent = arguments[2];
    // A load waits as long as it takes, and a failure here would too.
    std::thread([] {
        std::this_thread::sleep_for(60s);
        std::puts("timed out");
        std::fflush(stdout);
        std::_Exit(1);
    }).detach();

    // A failure on the way, and the picture still comes.
    EXPECT(Get(base + "/breaks/1").get() != nullptr);
    EXPECT(Asked("/breaks/1") >= 3);
    EXPECT(Get(base + "/busy/1").get() != nullptr);
    EXPECT(Asked("/busy/1") == 3);

    // A connection that stalls reports nothing, and is given up on: the
    // server stays silent for 20 seconds, and a try may wait for 1.
    auto stalled = std::chrono::steady_clock::now();
    EXPECT(Get(base + "/stalls/1").get() != nullptr);
    EXPECT(Asked("/stalls/1") == 2);
    EXPECT(std::chrono::steady_clock::now() - stalled < 10s);

    // Nothing listens at first, as while a proxy starts.
    auto starting = Get("http://127.0.0.1:" + silent + "/picture");
    std::this_thread::sleep_for(300ms);
    Text("/listen/" + silent);
    TryAgain();
    EXPECT(starting.get() != nullptr);

    // Tries end, and a load rests until something changes.
    auto down = Get(base + "/down/1");
    int tried = Settled("/down/1");
    EXPECT(tried >= 3);
    TryAgain();
    EXPECT(Settled("/down/1") > tried);
    Text("/heal/down/1");
    NetworkChanged(true);
    EXPECT(down.get() != nullptr);

    // A view that goes away stops the tries, and one that appears again
    // starts over.
    auto view = std::make_shared<Waiter>();
    auto gone = Get(base + "/down/2", view);
    std::this_thread::sleep_for(10ms);
    view->Left();
    int left = Settled("/down/2");
    std::this_thread::sleep_for(300ms);
    TryAgain();
    std::this_thread::sleep_for(300ms);
    EXPECT(Asked("/down/2") == left);
    Text("/heal/down/2");
    view->Appeared();
    EXPECT(gone.get() != nullptr);

    // A view that ends ends its load.
    auto ended = std::make_shared<Waiter>();
    auto finished = Get(base + "/down/3", ended);
    std::this_thread::sleep_for(10ms);
    ended->End();
    EXPECT(finished.get() == nullptr);

    // An answer that is no picture is not asked for again: not by the same
    // view, a new one or a window coming to the front.
    for (std::string path : {"/gone/1", "/page/1"}) {
        auto first = std::make_shared<Waiter>();
        auto waiting = Get(base + path, first);
        EXPECT(Settled(path) == 1);
        first->End();
        EXPECT(waiting.get() == nullptr);
        auto second = Get(base + path);
        TryAgain();
        EXPECT(Settled(path) == 1);
        // On another network it may have one after all.
        Text("/heal" + path);
        NetworkChanged(true);
        EXPECT(second.get() != nullptr);
        EXPECT(Asked(path) == 2);
    }
    std::puts("ok");
    return 0;
}
"#;

#[cfg(windows)]
#[test]
fn pictures_come_after_failures_and_refusals_are_not_repeated() {
    use std::path::Path;
    use std::process::Command;

    let vswhere = Path::new(&std::env::var("ProgramFiles(x86)").unwrap_or_default())
        .join("Microsoft Visual Studio/Installer/vswhere.exe");
    if !vswhere.is_file() {
        eprintln!("the Visual Studio C++ tools are not installed: the C++ was not run");
        return;
    }
    let found = Command::new(vswhere)
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .output()
        .unwrap();
    let studio = String::from_utf8_lossy(&found.stdout).trim().to_string();
    let vcvars = Path::new(&studio).join("VC/Auxiliary/Build/vcvarsall.bat");
    let architecture = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/picture-retry-winui");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.cpp"), TESTS).unwrap();
    std::fs::write(
        root.join("bern_pictures.h"),
        include_str!("../templates/cpp/bern_pictures.h"),
    )
    .unwrap();
    std::fs::write(
        root.join("build.bat"),
        format!(
            "@call \"{}\" {architecture} >nul || exit /b 1\r\n\
             cl /nologo /std:c++20 /EHsc /permissive- /utf-8 \
             /DBERN_PICTURE_WAITS=0.02,0.02 /DBERN_PICTURE_SILENCE=1 \
             /I\"%WindowsSdkDir%Include\\%WindowsSDKVersion%cppwinrt\" main.cpp /Fe:picture-retry.exe\r\n",
            vcvars.display()
        ),
    )
    .unwrap();
    let built = Command::new("cmd")
        .args(["/d", "/c", "build.bat"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stdout)
    );
    let (address, silent) = faulty_server::start();
    let result = Command::new(root.join("picture-retry.exe"))
        .arg(format!("http://{address}"))
        .arg(silent.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
