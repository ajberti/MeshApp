import SwiftUI

@main
struct MeshIosApp: App {
    @StateObject private var runtime: MeshRuntime

    init() {
        let created: MeshRuntime
        do {
            created = try MeshRuntime()
        } catch {
            fatalError("MeshEngine failed to open: \(error)")
        }
        _runtime = StateObject(wrappedValue: created)
    }

    var body: some Scene {
        WindowGroup {
            ConversationsView()
                .environmentObject(runtime)
        }
    }
}
