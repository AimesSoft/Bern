// Chooses the front end as the app starts. Generated build input; do not edit.
//
// An app of both front ends runs this. The front end of iOS 17 and later is
// a framework of the bundle, loaded on those systems only: what it links
// does not exist on iOS 16. This executable is the front end of iOS 16.

import Foundation

@main
enum BernLaunch {
    @MainActor static func main() {
        if #available(iOS 17.0, *), ProcessInfo.processInfo.environment["BERN_FRONT_END"] != "16",
           let frameworks = Bundle.main.privateFrameworksPath,
           let image = dlopen(frameworks + "/BernUI17.framework/BernUI17", RTLD_NOW),
           let entry = dlsym(image, "bern_ui17_main") {
            unsafeBitCast(entry, to: (@convention(c) () -> Void).self)()
        }
        BernApplication.main()
    }
}
