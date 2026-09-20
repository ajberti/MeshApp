import SwiftUI

struct ConversationView: View {
    @EnvironmentObject private var runtime: MeshRuntime
    let contact: MeshContact
    @State private var draft = ""

    private var thread: [MeshMessage] {
        runtime.messages(for: contact)
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 10) {
                        ForEach(thread, id: \.messageId) { message in
                            MessageBubble(message: message)
                                .id(message.messageId)
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 16)
                }
                .onChange(of: thread.count) { _, _ in
                    scrollToLatest(proxy)
                }
                .onAppear {
                    scrollToLatest(proxy)
                }
            }
            if let errorText = runtime.errorText {
                Text(errorText)
                    .font(.footnote)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.top, 8)
            }
            Divider()
            HStack(alignment: .bottom, spacing: 10) {
                TextField("Message", text: $draft, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...5)
                Button {
                    let text = draft
                    draft = ""
                    runtime.send(text: text, to: contact)
                } label: {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 32))
                }
                .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityLabel("Send")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
        }
        .background(Color(.systemGroupedBackground))
        .navigationTitle(contact.title)
        .navigationBarTitleDisplayMode(.inline)
    }

    private func scrollToLatest(_ proxy: ScrollViewProxy) {
        guard let last = thread.last else { return }
        DispatchQueue.main.async {
            withAnimation(.easeOut(duration: 0.2)) {
                proxy.scrollTo(last.messageId, anchor: .bottom)
            }
        }
    }
}

private struct MessageBubble: View {
    let message: MeshMessage

    private var outbound: Bool {
        message.direction == .outbound
    }

    var body: some View {
        HStack {
            if outbound { Spacer(minLength: 48) }
            VStack(alignment: outbound ? .trailing : .leading, spacing: 4) {
                Text(message.text)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(outbound ? Color.accentColor : Color(.secondarySystemGroupedBackground))
                    .foregroundStyle(outbound ? Color.white : Color.primary)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                if outbound {
                    Text(message.state.statusLabel)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            if !outbound { Spacer(minLength: 48) }
        }
        .frame(maxWidth: .infinity, alignment: outbound ? .trailing : .leading)
    }
}
