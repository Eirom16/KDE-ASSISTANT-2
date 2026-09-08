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
    signal newSessionClicked()
    signal settingsClicked()

    color: Qt.rgba(
        isDark ? 0.11 : 0.96,
        isDark ? 0.11 : 0.96,
        isDark ? 0.12 : 0.97,
        0.85
    )

    border.width: 1
    border.color: Theme.hairline

    Behavior on x {
        NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
    }

    // === Header ===
    Item {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.spacingMd
        height: 36

        PillButton {
            id: newBtn
            text: qsTr("Nueva sesion")
            iconName: "plus-16"
            active: false
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            onClicked: root.newSessionClicked()
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

        Column {
            width: parent.width
            spacing: Theme.spacingXs

                Repeater {
                    model: root.sessions
                    delegate: Item {
                        required property var modelData
                        width: root.drawerWidth - Theme.spacingMd * 2
                        height: 36

                        RowLayout {
                            anchors.fill: parent
                            spacing: Theme.spacingXs

                            PillButton {
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
                                onClicked: root.sessionSelected(modelData.id)
                            }

                            IconButton {
                                Layout.preferredWidth: 28
                                Layout.preferredHeight: 28
                                Layout.alignment: Qt.AlignVCenter
                                iconName: "trash-16"
                                iconSize: 14
                                buttonSize: 28
                                backgroundColor: "transparent"
                                iconColor: Theme.inkMuted
                                onClicked: root.sessionDeleteRequested(modelData.id)
                            }
                        }
                    }
                }

            // Empty state
            Column {
                width: parent.width
                spacing: Theme.spacingXs
                visible: root.sessions.length === 0

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

        PillButton {
            text: qsTr("Configuracion")
            iconName: "gear-16"
            anchors.horizontalCenter: parent.horizontalCenter
            onClicked: root.settingsClicked()
        }
    }
}
