// Bern runtime for Windows: the application's interface in the system XAML
// front end, through XAML Islands. The runtime comes from the precompiled
// header, so this is what a build compiles when the interface changes.
// Generated build input; do not edit.

#include "bern_pch.h"

#define BERN_WINUI 0
namespace bern::islands {
#include "app_ui.inl"
}
#undef BERN_WINUI

int BernRunIslands() {
    static bern::islands::AppDef const definition{bern::BernCatalog(), bern::islands::Windows(), bern::islands::AccentColors(), bern::islands::StatusItem()};
    return bern::islands::Run(definition);
}
