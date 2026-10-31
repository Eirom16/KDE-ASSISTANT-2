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
    property bool speechAvailable: false
    property var messages: []   // Array de {role, authorLabel, content, timestamp, isStreaming, toolCalls}
    property bool showWelcome: messages.length === 0
    property var previousMessages: []

    // Main reemplaza el array al recibir cada delta. Mantener estable el modelo
    // evita destruir todas las burbujas visibles por un cambio en la última.
    ListModel { id: messageRows; dynamicRoles: true }
    onMessagesChanged: syncMessages()
    Component.onCompleted: syncMessages()
    function syncMessages() {
        var next = messages || []
        if (next.length < messageRows.count) messageRows.clear()
        var common = Math.min(next.length, messageRows.count)
        for (var i = 0; i < common; i++) {
            if (previousMessages[i] !== next[i])
                messageRows.setProperty(i, "modelData", next[i])
        }
        for (var j = common; j < next.length; j++)
            messageRows.append({ modelData: next[j] })
        previousMessages = next
    }

    signal suggestionClicked(string text)
    signal imageClicked(string url, string caption)
    signal toolRetryClicked(string toolCallId)
    signal copyRequested(string text)
    signal playRequested(string text)
    signal regenerateRequested()
    signal toolApproveRequested(string toolCallId)
    signal toolDenyRequested(string toolCallId)
    signal toolDetailRequested(string toolName, string result)

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
            objectName: "messageList"
            anchors.fill: parent
            anchors.leftMargin: Theme.spacingMd
            // ScrollBar pegado a la esquina (6px del borde ventana).
            // El aire del contenido lo da el delegado (bubble -20px),
            // no el margen: asi la pista no flota en medio del chat.
            anchors.rightMargin: 6
            anchors.topMargin: Theme.spacingLg
            anchors.bottomMargin: Theme.spacingLg
            visible: !chatRoot.showWelcome
            clip: true
            model: messageRows
            spacing: Theme.spacingLg
            cacheBuffer: 800
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
                width: 6
                padding: 0
                // Sin pista ni linea separadora: solo el thumb flotante.
                background: Item {}
                contentItem: Rectangle {
                    radius: 3
                    color: Theme.inkMuted
                    opacity: parent.active ? 0.65 : 0.35
                    Behavior on opacity {
                        NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic }
                    }
                }
            }
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
                    // Centrada con ancho máximo (evita hueco en ventana ancha).
                    // Se restan 20px (10 por lado): el ScrollBar overlay de 6px
                    // queda en el gutter sin tapar burbuja ni "Tu"/timestamp.
                    width: Math.min(parent.width - 20, Theme.maxContentWidth)
                    anchors.horizontalCenter: parent.horizontalCenter
                    speechAvailable: chatRoot.speechAvailable
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
                    onToolApproveRequested: function(id) { chatRoot.toolApproveRequested(id) }
                    onToolDenyRequested: function(id) { chatRoot.toolDenyRequested(id) }
                    onToolDetailRequested: function(name, result) { chatRoot.toolDetailRequested(name, result) }
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
