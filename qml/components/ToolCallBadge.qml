// ToolCallBadge.qml - Pill informativa de accion ejecutada por el agente
// Estados: running (rotating), success (verde), error (rojo), confirm (azul)

import QtQuick
import qml 1.0

Rectangle {
    id: root

    property string toolName: ""
    property string status: "running"  // "running" | "success" | "error" | "confirm"
    property string result: ""          // preview del resultado
    property string imageUrl: ""

    signal imageClicked(string url)
    signal retryClicked()
    signal approveClicked()
    signal denyClicked()
    signal detailRequested()

    property int iconSize: Theme.iconSizeSm

        implicitWidth: row.implicitWidth + paddingH * 2
    implicitHeight: row.implicitHeight + paddingV * 2
    radius: Theme.radiusPill

    property int paddingH: Theme.spacingSm
    property int paddingV: Theme.spacingXs - 1

    color: {
        if (status === "success") return Theme.successTint
        if (status === "error") return Theme.errorTint
        if (status === "confirm") return Theme.primaryTint
        return Theme.surfacePearl
    }

    border.width: 1
    border.color: {
        if (status === "success") return Qt.rgba(Theme.success.r, Theme.success.g, Theme.success.b, 0.3)
        if (status === "error") return Qt.rgba(Theme.error.r, Theme.error.g, Theme.error.b, 0.3)
        if (status === "confirm") return Qt.rgba(Theme.primary.r, Theme.primary.g, Theme.primary.b, 0.4)
        return Theme.hairline
    }

    Behavior on color {
        ColorAnimation { duration: Theme.animNormal }
    }

    // Fondo clicable DETRÁS del contenido: imagen o detalle del resultado.
    // (Los botones internos están encima y consumen sus clicks primero.)
    MouseArea {
        anchors.fill: parent
        cursorShape: (root.imageUrl !== "" || (root.result !== "" && root.status !== "confirm" && root.status !== "running")) ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: {
            if (root.imageUrl !== "") {
                root.imageClicked(root.imageUrl)
            } else if (root.result !== "" && root.status !== "confirm" && root.status !== "running") {
                root.detailRequested()
            }
        }
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
                    if (root.toolName === "find_file") return "search-16"
                    if (root.toolName === "open_file") return "file-16"
                    if (root.toolName === "open_url") return "link-16"
                    if (root.toolName === "system_info") return "terminal-16"
                    if (root.toolName === "notify") return "bell-16"
                    if (root.toolName === "media") return "play-16"
                    if (root.toolName === "volume") return "unmute-16"
                    if (root.toolName === "network_status") return "terminal-16"
                    if (root.toolName === "remind_in") return "bell-16"
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
                    if (root.status === "confirm") return root.toolName + qsTr(" · ¿ejecutar?")
                    return root.toolName
                }
                font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, -0.05)
                color: {
                    if (root.status === "success") return Theme.success
                    if (root.status === "error") return Theme.error
                    if (root.status === "confirm") return Theme.primary
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

        // Boton de retry cuando hay error
        Item {
            visible: root.status === "error"
            width: visible ? 20 : 0
            height: 20
            anchors.verticalCenter: parent.verticalCenter

            Octicon {
                anchors.fill: parent
                name: "sync-16"
                size: 14
                color: Theme.error
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: root.retryClicked()
            }
        }

        // Aprobar / denegar cuando pide confirmación (F4-1)
        Item {
            visible: root.status === "confirm"
            width: visible ? 48 : 0
            height: 24
            anchors.verticalCenter: parent.verticalCenter

            Row {
                anchors.centerIn: parent
                spacing: 4

                Rectangle {
                    width: 22; height: 22; radius: 11
                    color: Theme.success
                    Octicon {
                        anchors.centerIn: parent
                        name: "check-16"
                        size: 12
                        color: "#ffffff"
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.approveClicked()
                    }
                }
                Rectangle {
                    width: 22; height: 22; radius: 11
                    color: Theme.error
                    Octicon {
                        anchors.centerIn: parent
                        name: "x-16"
                        size: 12
                        color: "#ffffff"
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.denyClicked()
                    }
                }
            }
        }
    }
}
