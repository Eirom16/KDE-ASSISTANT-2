// SessionDrawer.qml - Panel lateral con sesiones en chips pill
// Animacion slide 150ms al abrir/cerrar

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0

Rectangle {
    id: root

    // === Props ===
    property bool drawerOpen: false
    property int drawerWidth: Theme.drawerWidth
    property var sessions: []   // Array de {id, title, updated_at, message_count}
    property string currentId: ""
    property bool isDark: true

    signal sessionSelected(string id)
    signal sessionDeleteRequested(string id)
    signal sessionRenameRequested(string id, string title)
    signal newSessionClicked()
    signal settingsClicked()
    signal auditRequested()

    // F1-2: búsqueda + edición inline de título.
    property string filter: ""
    property string editingId: ""
    property string editingText: ""

    function fmtDate(iso) {
        if (!iso) return ""
        try {
            var d = new Date(iso)
            if (isNaN(d.getTime())) return ""
            var dd = ("0" + d.getDate()).slice(-2)
            var mm = ("0" + (d.getMonth() + 1)).slice(-2)
            return dd + "/" + mm + " " + Qt.formatTime(d, "hh:mm")
        } catch (e) { return "" }
    }

    function filteredSessions() {
        var f = (root.filter || "").trim().toLowerCase()
        if (!f) return root.sessions
        return (root.sessions || []).filter(function(s) {
            return String(s.title || "").toLowerCase().indexOf(f) >= 0
        })
    }

    // F0-7: borrado con confirmación (doble click): primer click arma, segundo borra.
    property string confirmDeleteId: ""
    Timer {
        id: confirmTimer
        interval: 3000
        repeat: false
        onTriggered: root.confirmDeleteId = ""
    }

    color: Theme.canvas

    border.width: 1
    border.color: Theme.hairline

    // Sin animacion en el binding inicial (x: 0 -> -drawerWidth): el drawer
    // debe arrancar cerrado sin flash de apertura. Solo animar toggles.
    property bool animateX: false
    Behavior on x {
        enabled: root.animateX
        NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
    }
    Component.onCompleted: Qt.callLater(function() { root.animateX = true })

    // === Header ===
    Item {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.spacingMd
        height: 84

        PillButton {
            id: newBtn
            text: qsTr("Nueva sesion")
            iconName: "plus-16"
            active: false
            anchors.left: parent.left
            anchors.top: parent.top
            onClicked: root.newSessionClicked()
        }

        // F1-2: buscar sesiones por título.
        TextField {
            id: searchField
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 36
            placeholderText: qsTr("Buscar…")
            text: root.filter
            onTextChanged: root.filter = text
            font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, 0)
            color: Theme.ink
            placeholderTextColor: Theme.inkMuted
            background: Rectangle {
                radius: Theme.radiusMd
                color: Theme.surfacePearl
                border.width: 1
                border.color: searchField.activeFocus ? Theme.primary : Theme.hairline
            }
        }
    }

    // === Lista de sesiones ===
    ScrollView {
        anchors.top: header.bottom
        anchors.topMargin: Theme.spacingSm
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: footer.top
        anchors.margins: Theme.spacingSm
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

        Column {
            width: parent.width
            spacing: Theme.spacingXs

                Repeater {
                    model: filteredSessions()
                    delegate: Item {
                        required property var modelData
                        width: root.drawerWidth - Theme.spacingMd * 2
                        height: 56

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 36
                                spacing: Theme.spacingXs

                                // Modo edición inline (F1-2).
                                TextField {
                                    visible: root.editingId === modelData.id
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    text: root.editingId === modelData.id ? root.editingText : ""
                                    onTextChanged: if (root.editingId === modelData.id) root.editingText = text
                                    font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, 0)
                                    color: Theme.ink
                                    placeholderText: qsTr("Nombre…")
                                    Keys.onReturnPressed: {
                                        root.sessionRenameRequested(modelData.id, root.editingText)
                                        root.editingId = ""
                                    }
                                    Keys.onEscapePressed: root.editingId = ""
                                    Component.onCompleted: if (visible) forceActiveFocus()
                                    background: Rectangle {
                                        radius: Theme.radiusMd
                                        color: Theme.surface
                                        border.width: 1
                                        border.color: Theme.primary
                                    }
                                }
                                IconButton {
                                    visible: root.editingId === modelData.id
                                    Layout.preferredWidth: 28
                                    Layout.preferredHeight: 28
                                    Layout.alignment: Qt.AlignVCenter
                                    iconName: "check-16"
                                    iconSize: 14
                                    buttonSize: 28
                                    backgroundColor: Theme.primary
                                    iconColor: Theme.inkOnPrimary
                                    onClicked: {
                                        root.sessionRenameRequested(modelData.id, root.editingText)
                                        root.editingId = ""
                                    }
                                }

                                PillButton {
                                    visible: root.editingId !== modelData.id
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    text: {
                                        var t = modelData.title || qsTr("Sin titulo")
                                        var n = modelData.message_count || 0
                                        return n > 0 ? t + "  ·  " + n : t
                                    }
                                    iconName: modelData.id === root.currentId ? "hubot-16" : "history-16"
                                    active: modelData.id === root.currentId
                                    textSize: Theme.fontSizeBodySmall
                                    elideText: true
                                    onClicked: root.sessionSelected(modelData.id)
                                }

                                IconButton {
                                    visible: root.editingId !== modelData.id
                                    Layout.preferredWidth: 28
                                    Layout.preferredHeight: 28
                                    Layout.alignment: Qt.AlignVCenter
                                    iconName: "pencil-16"
                                    iconSize: 13
                                    buttonSize: 28
                                    backgroundColor: "transparent"
                                    iconColor: Theme.inkMuted
                                    onClicked: {
                                        root.editingId = modelData.id
                                        root.editingText = modelData.title || ""
                                    }
                                }

                                IconButton {
                                    visible: root.editingId !== modelData.id
                                    Layout.preferredWidth: 28
                                    Layout.preferredHeight: 28
                                    Layout.alignment: Qt.AlignVCenter
                                    iconName: root.confirmDeleteId === modelData.id ? "alert-16" : "trash-16"
                                    iconSize: 14
                                    buttonSize: 28
                                    backgroundColor: root.confirmDeleteId === modelData.id ? Theme.errorTint : "transparent"
                                    iconColor: root.confirmDeleteId === modelData.id ? Theme.error : Theme.inkMuted
                                    onClicked: {
                                        if (root.confirmDeleteId === modelData.id) {
                                            root.confirmDeleteId = ""
                                            confirmTimer.stop()
                                            root.sessionDeleteRequested(modelData.id)
                                        } else {
                                            root.confirmDeleteId = modelData.id
                                            confirmTimer.restart()
                                        }
                                    }
                                }
                            }

                            // Fecha real + contador (F1-1).
                            Text {
                                visible: root.editingId !== modelData.id
                                text: fmtDate(modelData.updated_at) + (modelData.message_count ? "  ·  " + modelData.message_count + qsTr(" msgs") : "")
                                font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                                color: Theme.inkMuted
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                leftPadding: 34
                            }
                        }
                    }
                }

            // Empty state
            Column {
                width: parent.width
                spacing: Theme.spacingXs
                visible: {
                    var list = filteredSessions()
                    if ((root.filter || "").trim() !== "" && list.length === 0) return false
                    return root.sessions.length === 0
                }

                Octicon {
                    name: "history-16"
                    size: 32
                    color: Theme.inkMuted
                    opacity: 0.4
                    anchors.horizontalCenter: parent.horizontalCenter
                }
                Text {
                    text: qsTr("Sin sesiones")
                    font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                    color: Theme.inkMuted
                    anchors.horizontalCenter: parent.horizontalCenter
                }
            }
        }
    }

    // === Footer ===
    Item {
        id: footer
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.spacingMd
        height: 36

        RowLayout {
            anchors.fill: parent
            spacing: Theme.spacingXs

            PillButton {
                Layout.fillWidth: true
                Layout.fillHeight: true
                text: qsTr("Configuracion")
                iconName: "gear-16"
                textSize: Theme.fontSizeBodySmall
                onClicked: root.settingsClicked()
            }

            IconButton {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                Layout.alignment: Qt.AlignVCenter
                iconName: "history-16"
                iconSize: 16
                buttonSize: 36
                backgroundColor: "transparent"
                iconColor: Theme.inkMuted
                onClicked: root.auditRequested()
            }
        }
    }
}
