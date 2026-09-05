// ChatView.qml - ScrollView con todas las burbujas del chat
// Auto-scroll al final cuando llega un nuevo mensaje
// Soporta WelcomeScreen cuando no hay mensajes

import QtQuick
import QtQuick.Controls
import qml 1.0

Item {
    id: root

    // === Props ===
    property var messages: []   // Array de {role, authorLabel, content, timestamp, isStreaming, toolCalls}
    property bool showWelcome: messages.length === 0

    signal suggestionClicked(string text)
    signal imageClicked(string url, string caption)
    signal toolRetryClicked(string toolCallId)

    Rectangle {
        anchors.fill: parent
        color: "transparent"

        // Welcome screen cuando no hay mensajes
        WelcomeScreen {
            anchors.fill: parent
            visible: root.showWelcome
            onSuggestionClicked: function(text) { root.suggestionClicked(text) }
        }

        // ScrollView con mensajes
        ScrollView {
            id: scroll
            anchors.fill: parent
            visible: !root.showWelcome
            clip: true

            ScrollBar.vertical.policy: ScrollBar.AsNeeded

            Column {
                width: scroll.width
                height: childrenRect.height
                spacing: Theme.spacingLg
                topPadding: Theme.spacingLg
                bottomPadding: Theme.spacingLg
                leftPadding: Theme.spacingMd
                rightPadding: Theme.spacingMd

                Repeater {
                    model: root.messages
                    delegate: MessageBubble {
                        required property var modelData
                        width: scroll.width - Theme.spacingMd * 2
                        role: modelData.role || "assistant"
                        authorLabel: modelData.authorLabel || ""
                        content: modelData.content || ""
                        timestamp: modelData.timestamp || ""
                        isStreaming: modelData.isStreaming || false
                        toolCalls: modelData.toolCalls || []
                        onImageClicked: function(url) {
                            var cap = ""
                            for (var i = 0; i < (root.messages ? root.messages.length : 0); i++) {
                                var tcs = root.messages[i].toolCalls || []
                                for (var j = 0; j < tcs.length; j++) {
                                    if (tcs[j].imageUrl === url) {
                                        cap = tcs[j].caption || ""
                                        break
                                    }
                                }
                            }
                            root.imageClicked(url, cap)
                        }
                    }
                }

                // Typing indicator al final cuando streaming
                TypingIndicator {
                    width: 60
                    height: 20
                    visible: root.messages.length > 0 &&
                             root.messages[root.messages.length - 1] &&
                             root.messages[root.messages.length - 1].isStreaming
                    active: visible
                }
            }
        }
    }
}
