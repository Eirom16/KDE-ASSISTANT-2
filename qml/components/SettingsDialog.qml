// SettingsDialog.qml - Dialog modal de configuracion
// Acceso desde el SessionDrawer ("Configuracion") o desde la X del title

import QtQuick
import QtQuick.Controls
import qml 1.0
import QtQuick.Layouts
import "."

Rectangle {
    id: root

    property bool open_: false
    property string apiKey: ""
    property string model: ""
    property string baseUrl: ""
    property bool toolCallingEnabled: true
    property bool autoSpeak: false
    property bool chimesEnabled: true
    property string theme: "system"  // "system" | "dark" | "light"

    signal closed()
    signal saved()

    visible: open_
    color: Qt.rgba(0, 0, 0, 0.5)
    z: 998

    function show() {
        root.open_ = true
    }
    function hide() {
        root.open_ = false
        root.closed()
    }

    MouseArea {
        anchors.fill: parent
        onClicked: root.hide()
    }

    // === Panel de settings ===
    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 480)
        height: Math.min(parent.height - 80, 580)
        radius: Theme.radiusLg + 2
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline

        // Sombra
        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            z: -1
            color: Theme.shadowCard
            opacity: 0.5
            radius: parent.radius + 4
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.spacingLg
            spacing: Theme.spacingMd

            // === Header ===
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.spacingXs

                Text {
                    text: qsTr("Configuracion")
                    font: Theme.font(Theme.fontSizeTagline, Theme.weightBold, -0.1)
                    color: Theme.ink
                    Layout.fillWidth: true
                }

                IconButton {
                    iconName: "x-16"
                    iconSize: 18
                    buttonSize: 32
                    backgroundColor: "transparent"
                    iconColor: Theme.ink
                    onClicked: root.hide()
                }
            }

            // === Tabs (placeholder simple con pills) ===
            Flow {
                Layout.fillWidth: true
                spacing: Theme.spacingXs
                PillButton {
                    text: qsTr("General")
                    active: true
                }
                PillButton {
                    text: qsTr("Voz")
                    active: false
                }
                PillButton {
                    text: qsTr("Atajos")
                    active: false
                }
            }

            // === Form ===
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: Theme.spacingMd

                    // AI section
                    Text {
                        text: qsTr("Inteligencia Artificial")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                    }

                    SettingsField {
                        label: qsTr("API URL")
                        value: root.baseUrl
                        placeholder: "https://openrouter.ai/api/v1"
                    }

                    SettingsField {
                        label: qsTr("API Key")
                        value: root.apiKey
                        placeholder: "sk-or-..."
                        isPassword: true
                    }

                    SettingsField {
                        label: qsTr("Modelo")
                        value: root.model
                        placeholder: "openrouter/z-ai/glm-5.2:free"
                    }

                    SettingsToggle {
                        label: qsTr("Tool calling (agente)")
                        description: qsTr("Permitir al asistente ejecutar herramientas")
                        active: root.toolCallingEnabled
                    }

                    // Tema
                    Text {
                        text: qsTr("Apariencia")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingMd
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        PillButton {
                            text: qsTr("Sistema")
                            active: root.theme === "system"
                            onClicked: root.theme = "system"
                        }
                        PillButton {
                            text: qsTr("Oscuro")
                            active: root.theme === "dark"
                            onClicked: root.theme = "dark"
                        }
                        PillButton {
                            text: qsTr("Claro")
                            active: root.theme === "light"
                            onClicked: root.theme = "light"
                        }
                    }

                    // Voz
                    Text {
                        text: qsTr("Voz")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingMd
                    }

                    SettingsToggle {
                        label: qsTr("Hablar respuestas automaticamente")
                        active: root.autoSpeak
                    }

                    SettingsToggle {
                        label: qsTr("Reproducir chimes")
                        description: qsTr("Sonidos sutiles al activar/desactivar voz")
                        active: root.chimesEnabled
                    }
                }
            }

            // === Footer ===
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.spacingXs

                PillButton {
                    text: qsTr("Cancelar")
                    onClicked: root.hide()
                }
                Item { Layout.fillWidth: true }
                AppleButton {
                    text: qsTr("Guardar")
                    iconName: "check-16"
                    variant: "primary"
                    onClicked: {
                        root.saved()
                        root.hide()
                    }
                }
            }
        }

        // Bloquear propagacion de clicks al backdrop
        MouseArea {
            anchors.fill: parent
            onClicked: {} // absorb
        }
    }
}
