import SwiftUI

struct AdvancedView: View {
    @EnvironmentObject private var runtime: MeshRuntime

    var body: some View {
        List {
            Section("This device") {
                LabeledContent("Fingerprint") {
                    Text(runtime.fingerprint)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                }
                LabeledContent("Discovery ID") {
                    Text(runtime.discoveryIdHex)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                }
                Text(runtime.contactCard)
                    .font(.caption.monospaced())
                    .textSelection(.enabled)
                Button("Copy contact card") {
                    runtime.copyContactCard()
                }
            }
            Section("Nearby radios") {
                if runtime.peers.isEmpty {
                    Text("No Bonjour peers on _mesh._tcp")
                        .foregroundStyle(.secondary)
                }
                ForEach(runtime.peers, id: \.token) { peer in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(peer.displayName)
                        Text(peer.discoveryId.hexString)
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                    }
                }
            }
            Section("Engine log") {
                if runtime.logs.isEmpty {
                    Text("No log lines yet")
                        .foregroundStyle(.secondary)
                }
                ForEach(Array(runtime.logs.enumerated()), id: \.offset) { _, line in
                    Text(line)
                        .font(.caption.monospaced())
                }
            }
        }
        .navigationTitle("Advanced")
        .navigationBarTitleDisplayMode(.inline)
    }
}
