import SwiftUI

struct AddContactView: View {
    @EnvironmentObject private var runtime: MeshRuntime
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        List {
            Section {
                Text("Your code")
                    .font(.headline)
                Text(runtime.fingerprint.meshShortFingerprint)
                    .font(.title3.monospaced())
                    .frame(maxWidth: .infinity)
                if let qrImage = runtime.qrImage {
                    Image(uiImage: qrImage)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(maxWidth: 240, maxHeight: 240)
                        .frame(maxWidth: .infinity)
                        .listRowInsets(EdgeInsets(top: 12, leading: 12, bottom: 12, trailing: 12))
                }
                Text("The other person must add you, and you must add them, before either of you can read messages.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Button("Copy contact card") {
                    runtime.copyContactCard()
                }
            }
            Section("Add someone") {
                if ContactScannerSheet.isAvailable {
                    Button("Scan their QR code") {
                        runtime.showScanner = true
                    }
                } else {
                    Text("This simulator has no camera. Copy their card from the other simulator and paste it below.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                TextField("Paste mesh:1 card", text: $runtime.pasteText, axis: .vertical)
                    .font(.caption.monospaced())
                    .lineLimit(3...8)
                Button("Add pasted contact") {
                    runtime.importPastedContact()
                }
                .disabled(runtime.pasteText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            if let errorText = runtime.errorText {
                Section {
                    Text(errorText)
                        .foregroundStyle(.red)
                }
            }
            if !runtime.contacts.isEmpty {
                Section("Contacts") {
                    ForEach(runtime.contacts, id: \.userId) { contact in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(contact.title)
                            Text(contact.fingerprint.meshShortFingerprint)
                                .font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("Add contact")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Done") { dismiss() }
            }
        }
        .onChange(of: runtime.contacts.count) { oldCount, newCount in
            if newCount > oldCount {
                dismiss()
            }
        }
        .sheet(isPresented: $runtime.showScanner) {
            ContactScannerSheet { payload in
                runtime.importContactCard(payload)
            }
        }
    }
}
