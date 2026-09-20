import SwiftUI

struct ConversationsView: View {
    @EnvironmentObject private var runtime: MeshRuntime

    var body: some View {
        NavigationStack {
            Group {
                if runtime.contacts.isEmpty {
                    ContentUnavailableView(
                        "No conversations",
                        systemImage: "bubble.left.and.bubble.right",
                        description: Text("Add a contact with a QR code or by pasting their card. Both people must add each other before messages can be read.")
                    )
                } else {
                    List(runtime.inboxContacts, id: \.userId) { contact in
                        NavigationLink(value: contact) {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(contact.title)
                                    .font(.headline)
                                Text(runtime.preview(for: contact))
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            .padding(.vertical, 4)
                        }
                    }
                }
            }
            .navigationTitle("Messages")
            .navigationDestination(for: MeshContact.self) { contact in
                ConversationView(contact: contact)
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    NavigationLink("Advanced") {
                        AdvancedView()
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        runtime.errorText = nil
                        runtime.showAddContact = true
                    } label: {
                        Image(systemName: "person.badge.plus")
                    }
                    .accessibilityLabel("Add contact")
                }
            }
            .sheet(isPresented: $runtime.showAddContact) {
                NavigationStack {
                    AddContactView()
                }
            }
            .sheet(isPresented: Binding(
                get: { runtime.needsUsername },
                set: { _ in }
            )) {
                UsernameSetupView()
            }
        }
    }
}
