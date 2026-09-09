// MessageBubble.qml - Burbuja de mensaje estilo Apple
// Radio uniforme de 18px, max-width 75%, alineado por rol

import QtQuick
import QtQuick.Layouts
import qml 1.0

Item {
    id: root

    // === Props ===
    property string role: "assistant"   // "user" | "assistant" | "system"
    property string authorLabel: ""      // "Tu" | "KDE Assistant" | ...
    property string content: ""
    /// Texto plano original (markdown del asistente / texto del usuario).
    /// Se usa para copiar y para TTS (el HTML de `content` no sirve).
    property string rawContent: ""
    property string timestamp: ""
    property bool isStreaming: false
    property var toolCalls: []           // Array de {name, args, status, result, imageUrl?}

    // Ancho maximo responsivo y simétrico: hasta 78% del ancho, tope 600.
    // (Antes user capaba a 380 y assistant a 580: ritmo desigual.)
    property int maxWidth: {
        var w = parent ? parent.width : 480
        if (w <= 0) w = 480
        return Math.min(600, Math.floor(w * 0.78))
    }
    property int avatarSize: 28

    signal imageClicked(string url)
    signal copyRequested(string text)
    signal playRequested(string text)

    implicitHeight: column.implicitHeight + 8
    implicitWidth: column.implicitWidth

    // === Layout ===
    ColumnLayout {
        id: column
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Theme.spacingXs

        // Author label + avatar (solo asistente o sistema)
        RowLayout {
            visible: root.authorLabel !== ""
            spacing: Theme.spacingXs
            Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft

            // Avatar para asistente
            Rectangle {
                visible: root.role !== "user"
                width: root.avatarSize
                height: width
                radius: width / 2
                color: Theme.primary
                Octicon {
                    anchors.centerIn: parent
                    name: "hubot-16"
                    size: 16
                    color: Theme.inkOnPrimary
                }
            }

            Text {
                text: root.authorLabel
                font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                color: Theme.inkMuted
                textFormat: Text.PlainText
            }
        }

        // Burbuja de texto
        Rectangle {
            id: bubble
            Layout.maximumWidth: root.maxWidth
            Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft
            radius: Theme.radiusLg
            color: root.role === "user" ? Theme.primary : Theme.surface
            border.width: root.role === "user" ? 0 : 1
            border.color: Theme.hairline

            // Sombra sutil
            Rectangle {
                anchors.fill: parent
                anchors.margins: -1
                color: "transparent"
                z: -1
                radius: parent.radius
                border.width: 0
            }

            implicitWidth: Math.min(contentText.implicitWidth + paddingH * 2, root.maxWidth)
            implicitHeight: contentText.implicitHeight + paddingV * 2

            property int paddingH: Theme.spacingMd - 2
            property int paddingV: Theme.spacingSm

            TextEdit {
                id: contentText
                anchors.fill: parent
                anchors.margins: bubble.paddingV
                anchors.leftMargin: bubble.paddingH
                anchors.rightMargin: bubble.paddingH
                text: root.content
                readOnly: true
                selectByMouse: true
                // F0-8: WrapAnywhere para que URLs/código largos no fuercen
                // scroll horizontal (estaba Wrap y desbordaba a 360px).
                wrapMode: TextEdit.WrapAnywhere
                textFormat: TextEdit.RichText
                font: Theme.font(
                    Theme.fontSizeBody,
                    Theme.weightNormal,
                    Theme.lsBody
                )
                color: root.role === "user" ? Theme.inkOnPrimary : Theme.ink

                // Cursor parpadeante cuando esta streameando
                cursorVisible: root.isStreaming
                cursorPosition: root.content.length
            }
        }

        // Tool call badges (debajo de la burbuja)
        Flow {
            visible: root.toolCalls && root.toolCalls.length > 0
            Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft
            Layout.maximumWidth: root.maxWidth
            spacing: Theme.spacingXs

            Repeater {
                model: root.toolCalls
                delegate: ToolCallBadge {
                    required property var modelData
                    toolName: modelData.name || ""
                    status: modelData.status || "running"
                    result: modelData.result || ""
                    imageUrl: modelData.imageUrl || ""
                    onImageClicked: root.imageClicked(modelData.imageUrl)
                }
            }
        }

        // Image card (si hay show_image)
        Repeater {
            model: root.toolCalls ? root.toolCalls.filter(function(tc) { return tc.imageUrl }) : []
            delegate: ImageCard {
                required property var modelData
                Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft
                Layout.maximumWidth: root.maxWidth
                source: modelData.imageUrl
                caption: modelData.caption || ""
                onClicked: root.imageClicked(modelData.imageUrl)
            }
        }

        // Timestamp
        Text {
            visible: root.timestamp !== ""
            text: root.timestamp
            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
            color: Theme.inkMuted
            Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft
        }

        // Acciones de respuesta (solo asistente, con texto plano disponible)
        RowLayout {
            visible: root.role === "assistant" && !root.isStreaming && root.rawContent !== ""
            spacing: Theme.spacingXs
            Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft

            IconButton {
                iconName: "copy-16"
                iconSize: 14
                buttonSize: 28
                backgroundColor: "transparent"
                iconColor: Theme.inkMuted
                onClicked: root.copyRequested(root.rawContent)
            }
            IconButton {
                iconName: "play-16"
                iconSize: 14
                buttonSize: 28
                backgroundColor: "transparent"
                iconColor: Theme.inkMuted
                onClicked: root.playRequested(root.rawContent)
            }
        }
    }
}
