import SwiftUI

struct DebugRootView: View {
    @EnvironmentObject private var runtime: MeshRuntime

    var body: some View {
        NavigationStack {
            List {
                Section("This device") {
                    Text(runtime.fingerprint)
                        .font(.footnote.monospaced())
                        .textSelection(.enabled)
                }
                Section("Peers") {
                    if runtime.peers.isEmpty {
                        Text("Waiting for Bonjour peers on _mesh._tcp")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(runtime.peers, id: \.token) { peer in
                        VStack(alignment: .leading) {
                            Text(peer.displayName)
                            Text(peer.discoveryId.hexString)
                                .font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Section("Messages") {
                    if runtime.messages.isEmpty {
                        Text("No plaintext messages yet")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(Array(runtime.messages.enumerated()), id: \.offset) { _, message in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(message.text)
                            Text(message.direction == .outbound ? "out" : "in")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Section("Send") {
                    TextField("Message", text: $runtime.draft)
                    Button("Send to first contact") {
                        runtime.sendDraft()
                    }
                    if let errorText = runtime.errorText {
                        Text(errorText)
                            .foregroundStyle(.red)
                            .font(.footnote)
                    }
                }
                Section("Log") {
                    ForEach(Array(runtime.logs.enumerated()), id: \.offset) { _, line in
                        Text(line)
                            .font(.caption.monospaced())
                    }
                }
            }
            .navigationTitle("Mesh debug")
        }
    }
}
