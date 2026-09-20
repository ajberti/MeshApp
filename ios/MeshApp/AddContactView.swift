import SwiftUI

struct AddContactView: View {
    @EnvironmentObject private var runtime: MeshRuntime
    @Environment(\.dismiss) private var dismiss
    @State private var usernameDraft = ""

    var body: some View {
        List {
            Section("Your name") {
                TextField("Username", text: $usernameDraft)
                    .textInputAutocapitalization(.words)
                    .autocorrectionDisabled()
                    .onSubmit {
                        saveUsername()
                    }
                if !runtime.username.isEmpty {
                    Text("People you share with will see “\(runtime.username)”.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            Section {
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
                    saveUsername()
                    runtime.copyContactCard()
                }
            }
            Section("Add someone") {
                if ContactScannerSheet.isAvailable {
                    Button("Scan their QR code") {
                        saveUsername()
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
                        Text(contact.title)
                    }
                }
            }
        }
        .navigationTitle("Add contact")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Done") {
                    saveUsername()
                    dismiss()
                }
            }
        }
        .onAppear {
            usernameDraft = runtime.username
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

    private func saveUsername() {
        let trimmed = usernameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != runtime.username else { return }
        _ = runtime.setUsername(trimmed)
    }
}
