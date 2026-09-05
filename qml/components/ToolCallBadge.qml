// ToolCallBadge.qml - Pill informativa de accion ejecutada por el agente
// Estados: running (rotating), success (verde), error (rojo)

import QtQuick
import qml 1.0

Rectangle {
    id: root

    property string toolName: ""
    property string status: "running"  // "running" | "success" | "error"
    property string result: ""          // preview del resultado
    property string imageUrl: ""

    property int iconSize: Theme.iconSizeSm

    implicitWidth: row.implicitWidth + paddingH * 2
    implicitHeight: row.implicitHeight + paddingV * 2
    radius: Theme.radiusPill

    property int paddingH: Theme.spacingSm
    property int paddingV: Theme.spacingXs - 1

    color: {
        if (status === "success") return Theme.successTint
        if (status === "error") return Theme.errorTint
        return Theme.surfacePearl
    }

    border.width: 1
    border.color: {
        if (status === "success") return Qt.rgba(Theme.success.r, Theme.success.g, Theme.success.b, 0.3)
        if (status === "error") return Qt.rgba(Theme.error.r, Theme.error.g, Theme.error.b, 0.3)
        return Theme.hairline
    }

    Behavior on color {
        ColorAnimation { duration: Theme.animNormal }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.spacingXs

        // Icono segun herramienta y estado
        Item {
            width: root.iconSize
            height: root.iconSize
            anchors.verticalCenter: parent.verticalCenter

            // Icono principal
            Octicon {
                anchors.fill: parent
                name: {
                    if (root.status === "success") return "check-16"
                    if (root.status === "error") return "x-16"
                    if (root.toolName === "open_app") return "rocket-16"
                    if (root.toolName === "create_file") return "file-added-16"
                    if (root.toolName === "edit_file") return "pencil-16"
                    if (root.toolName === "read_file") return "file-16"
                    if (root.toolName === "web_search") return "search-16"
                    if (root.toolName === "show_image") return "image-16"
                    return "tools-16"
                }
                size: root.iconSize
                color: {
                    if (root.status === "success") return Theme.success
                    if (root.status === "error") return Theme.error
                    return Theme.inkMuted
                }

                // Rotacion cuando esta running
                RotationAnimation on rotation {
                    running: root.status === "running"
                    loops: Animation.Infinite
                    from: 0; to: 360
                    duration: 1500
                }
            }
        }

        // Texto
        Column {
            spacing: 0
            anchors.verticalCenter: parent.verticalCenter

            Text {
                text: {
                    if (root.status === "success") return root.toolName + " OK"
                    if (root.status === "error") return root.toolName + " (error)"
                    return root.toolName
                }
                font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, -0.05)
                color: {
                    if (root.status === "success") return Theme.success
                    if (root.status === "error") return Theme.error
                    return Theme.ink
                }
            }
            Text {
                visible: root.result !== ""
                text: root.result.length > 60 ? root.result.substring(0, 57) + "..." : root.result
                font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                color: Theme.inkMuted
                maximumLineCount: 1
                elide: Text.ElideRight
            }
        }
    }
}
