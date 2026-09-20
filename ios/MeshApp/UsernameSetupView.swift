import SwiftUI

struct UsernameSetupView: View {
    @EnvironmentObject private var runtime: MeshRuntime
    @State private var draft = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Username", text: $draft)
                        .textInputAutocapitalization(.words)
                        .autocorrectionDisabled()
                    Text("This name is shown to people you exchange contact cards with. It is not a unique ID.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                if let errorText = runtime.errorText {
                    Section {
                        Text(errorText)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Your name")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Continue") {
                        _ = runtime.setUsername(draft)
                    }
                    .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .onAppear {
                draft = runtime.username
            }
        }
        .interactiveDismissDisabled()
    }
}
