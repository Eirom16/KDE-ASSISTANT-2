// ChatView.qml - Lista virtualizada con todas las burbujas del chat
// ListView (no Repeater) para no instanciar 100+ burbujas a la vez.
// Auto-scroll al final solo si el usuario ya estaba abajo (follow).
// Soporta WelcomeScreen cuando no hay mensajes
// NOTA: el id es chatRoot (no "root") para que los delegates no lo confundan
// con el "root" interno de MessageBubble.

import QtQuick
import QtQuick.Controls
import qml 1.0

Item {
    id: chatRoot

    // === Props ===
    property var messages: []   // Array de {role, authorLabel, content, timestamp, isStreaming, toolCalls}
    property bool showWelcome: messages.length === 0

    signal suggestionClicked(string text)
    signal imageClicked(string url, string caption)
    signal toolRetryClicked(string toolCallId)
    signal copyRequested(string text)
    signal playRequested(string text)
    signal regenerateRequested()

    function findCaption(url) {
        var list = chatRoot.messages || []
        for (var i = 0; i < list.length; i++) {
            var tcs = list[i].toolCalls || []
            for (var j = 0; j < tcs.length; j++) {
                if (tcs[j].imageUrl === url) return tcs[j].caption || ""
            }
        }
        return ""
    }

    function scrollToEnd() {
        list.positionViewAtEnd()
    }

    Rectangle {
        anchors.fill: parent
        color: "transparent"

        // Welcome screen cuando no hay mensajes
        WelcomeScreen {
            anchors.fill: parent
            visible: chatRoot.showWelcome
            onSuggestionClicked: function(text) { chatRoot.suggestionClicked(text) }
        }

        // Lista virtualizada (visible cuando hay mensajes)
        ListView {
            id: list
            anchors.fill: parent
            anchors.leftMargin: Theme.spacingMd
            anchors.rightMargin: Theme.spacingMd + 8
            anchors.topMargin: Theme.spacingLg
            anchors.bottomMargin: Theme.spacingLg
            visible: !chatRoot.showWelcome
            clip: true
            model: chatRoot.messages
            spacing: Theme.spacingLg
            cacheBuffer: 800
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AlwaysOff }

            // Seguir abajo solo si el usuario ya estaba al final.
            property bool follow: true
            onAtYEndChanged: follow = atYEnd
            onMovementEnded: follow = atYEnd
            onFlickEnded: follow = atYEnd
            onCountChanged: {
                if (follow) Qt.callLater(function() { list.positionViewAtEnd() })
            }
            // Durante streaming el contenido crece: mantener abajo si follow.
            onContentHeightChanged: {
                var msgs = chatRoot.messages || []
                if (follow && msgs.length > 0) {
                    var last = msgs[msgs.length - 1]
                    if (last && last.isStreaming) {
                        Qt.callLater(function() { list.positionViewAtEnd() })
                    }
                }
            }

            delegate: Item {
                required property var modelData
                width: ListView.view.width
                // Altura de la burbuja interior
                implicitHeight: bubble.implicitHeight
                height: implicitHeight

                MessageBubble {
                    id: bubble
                    // Centrada con ancho máximo (evita hueco en ventana ancha)
                    width: Math.min(parent.width, Theme.maxContentWidth)
                    anchors.horizontalCenter: parent.horizontalCenter
                    role: modelData.role || "assistant"
                    authorLabel: modelData.authorLabel || ""
                    content: modelData.content || ""
                    rawContent: modelData.raw || ""
                    timestamp: modelData.timestamp || ""
                    isStreaming: modelData.isStreaming || false
                    toolCalls: modelData.toolCalls || []
                    onCopyRequested: function(text) { chatRoot.copyRequested(text) }
                    onPlayRequested: function(text) { chatRoot.playRequested(text) }
                    onRegenerateRequested: chatRoot.regenerateRequested()
                    onImageClicked: function(url) {
                        chatRoot.imageClicked(url, chatRoot.findCaption(url))
                    }
                }
            }

            footer: TypingIndicator {
                width: 60
                height: 20
                visible: {
                    var msgs = chatRoot.messages || []
                    return msgs.length > 0 && msgs[msgs.length - 1] && msgs[msgs.length - 1].isStreaming
                }
                active: visible
            }
        }
    }
}
