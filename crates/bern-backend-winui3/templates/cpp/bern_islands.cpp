// Bern runtime for Windows: the application's interface in the system XAML
// front end, through XAML Islands. The runtime comes from the precompiled
// header, so this is what a build compiles when the interface changes.
// Generated build input; do not edit.

#include "bern_pch.h"

namespace bern::islands {
#include "app_ui.inl"
}

int BernRunIslands() {
    static bern::islands::AppDef const definition{bern::BernCatalog(), bern::islands::Windows(), bern::islands::AccentColors()};
    return bern::islands::Run(definition);
}
