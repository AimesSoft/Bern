// Bern runtime for Windows: the application's interface in the WinUI 3
// front end. The runtime comes from the precompiled header, so this is what
// a build compiles when the interface changes. Generated build input; do
// not edit.

#include "bern_pch.h"

namespace bern::winui {
#include "app_ui.inl"
}

/// `resources`: the runtime's resource index, for a runtime that does not
/// lead WinUI to it; empty otherwise.
int BernRunWinUI(std::wstring const& resources) {
    static bern::winui::AppDef const definition{bern::BernCatalog(), bern::winui::Windows(), bern::winui::AccentColors(), bern::winui::StatusItem()};
    return bern::winui::Run(definition, resources);
}
