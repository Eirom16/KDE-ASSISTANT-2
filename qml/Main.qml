// Main.qml - Ventana principal KDE Assistant v2
// Estetica Apple Design (Cupertino), translucida, frosted glass

import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Layouts
import qml 1.0
import qml.components 1.0

ApplicationWindow {
    id: root

    title: "KDE Assistant v2"
    width: Theme.windowDefaultWidth
    height: Theme.windowDefaultHeight
    minimumWidth: Theme.windowMinWidth
    minimumHeight: Theme.windowMinHeight
    maximumWidth: Theme.windowMaxWidth
    maximumHeight: Theme.windowMaxHeight

    color: "transparent"   // El fondo lo da el Rectangle root

    flags: Qt.Window | Qt.WindowStaysOnTopHint
    visible: true

    // === Modelo de datos mock (en produccion vendra de Rust) ===
    property var mockSessions: [
        { id: "s1", title: "Conversacion sobre Rust", updatedAt: "hace 2h", count: 12 },
        { id: "s2", title: "Configurar KDE", updatedAt: "ayer", count: 5 },
        { id: "s3", title: "Organizacion de archivos", updatedAt: "hace 3d", count: 8 }
    ]

    property string currentSessionId: "s1"

    // Setter para alternar entre demo (welcome) y conversacion (con mensajes)
    property bool showDemoConversation: true

    property var mockMessages: showDemoConversation ? [
        {
            role: "user",
            authorLabel: "Tu",
            content: "Hola, puedes abrir Firefox?",
            timestamp: "10:42",
            isStreaming: false,
            toolCalls: []
        },
        {
            role: "assistant",
            authorLabel: "KDE Assistant",
            content: "Claro, abro Firefox ahora mismo.",
            timestamp: "10:42",
            isStreaming: false,
            toolCalls: [
                { name: "open_app", status: "success", result: "App 'firefox' abierta", imageUrl: "" }
            ]
        },
        {
            role: "user",
            authorLabel: "Tu",
            content: "Buscame el clima de hoy en Madrid",
            timestamp: "10:43",
            isStreaming: false,
            toolCalls: []
        },
        {
            role: "assistant",
            authorLabel: "KDE Assistant",
            content: "Estoy consultando... dame un momento.",
            timestamp: "10:43",
            isStreaming: false,
            toolCalls: [
                {
                    name: "web_search",
                    status: "success",
                    result: "Madrid 22°C, parcialmente nublado",
                    imageUrl: ""
                }
            ]
        },
        {
            role: "user",
            authorLabel: "Tu",
            content: "Muestrame una imagen del espacio",
            timestamp: "10:44",
            isStreaming: false,
            toolCalls: []
        },
        {
            role: "assistant",
            authorLabel: "KDE Assistant",
            content: "Aqui tienes una vista espectacular del espacio profundo captada por el telescopio Hubble.",
            timestamp: "10:44",
            isStreaming: false,
            toolCalls: [
                {
                    name: "show_image",
                    status: "success",
                    result: "Imagen: Pillars of Creation (NASA/ESA)",
                    imageUrl: "https://upload.wikimedia.org/wikipedia/commons/thumb/6/68/Pillars_2014_HST_WFC3-UVIS_full-res_denoised.jpg/800px-Pillars_2014_HST_WFC3-UVIS_full-res_denoised.jpg",
                    caption: "Pillars of Creation - Hubble"
                }
            ]
        },
        {
            role: "user",
            authorLabel: "Tu",
            content: "Crea un archivo notas.txt con la lista del super",
            timestamp: "10:46",
            isStreaming: false,
            toolCalls: []
        },
        {
            role: "assistant",
            authorLabel: "KDE Assistant",
            content: "Listo, archivo creado.",
            timestamp: "10:46",
            isStreaming: false,
            toolCalls: [
                {
                    name: "create_file",
                    status: "success",
                    result: "/home/user/Documentos/notas.txt",
                    imageUrl: ""
                }
            ]
        }
    ] : []

    // === Estado UI ===
    property bool drawerOpen: false
    property bool streaming: false
    property string voiceState: "idle"  // "idle" | "listening" | "processing" | "speaking"

    // === Background frosted ===
    Rectangle {
        id: bgRect
        anchors.fill: parent
        color: Theme.canvas
        radius: Theme.radiusLg + 4
        border.width: 1
        border.color: Theme.hairline
    }

    // === Layout principal ===
    Item {
        anchors.fill: parent
        anchors.margins: 1

        // === Drawer lateral (colapsable) ===
        SessionDrawer {
            id: drawer
            x: root.drawerOpen ? 0 : -drawer.drawerWidth
            y: 0
            width: drawer.drawerWidth
            height: parent.height
            drawerOpen: root.drawerOpen
            sessions: root.mockSessions
            currentId: root.currentSessionId
            isDark: Theme.isDark
            onNewSessionClicked: {
                root.drawerOpen = false
                root.currentSessionId = ""
            }
            onSessionSelected: function(id) {
                root.currentSessionId = id
                root.drawerOpen = false
            }
            onSettingsClicked: {
                root.drawerOpen = false
                settings.show()
            }
        }

        // === Main content area ===
        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: root.drawerOpen ? drawer.drawerWidth : 0
            anchors.rightMargin: 0
            anchors.topMargin: 0
            anchors.bottomMargin: 0
            spacing: 0

            Behavior on anchors.leftMargin {
                NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
            }

            // === Title bar ===
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 40

                // Toggle drawer
                IconButton {
                    id: toggleBtn
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: Theme.spacingXs
                    iconName: root.drawerOpen ? "sidebar-collapse-16" : "sidebar-expand-16"
                    iconSize: 18
                    buttonSize: 32
                    backgroundColor: "transparent"
                    iconColor: Theme.ink
                    onClicked: root.drawerOpen = !root.drawerOpen
                }

                // Title
                Text {
                    anchors.centerIn: parent
                    text: qsTr("KDE Assistant")
                    font: Theme.font(Theme.fontSizeBodyStrong, Theme.weightBold, -0.1)
                    color: Theme.ink
                }

                // Status indicator (conectado a backend)
                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.rightMargin: Theme.spacingMd
                    spacing: 6

                    Rectangle {
                        width: 6; height: 6; radius: 3
                        color: Theme.success
                        anchors.verticalCenter: parent.verticalCenter
                        SequentialAnimation on opacity {
                            loops: Animation.Infinite
                            NumberAnimation { from: 1.0; to: 0.4; duration: 1200 }
                            NumberAnimation { from: 0.4; to: 1.0; duration: 1200 }
                        }
                    }
                    Text {
                        text: qsTr("Online")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }

            // === Voice Orb (centrado, visible cuando no es idle) ===
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: root.voiceState === "idle" ? 0 : 180
                visible: root.voiceState !== "idle"

                Behavior on Layout.preferredHeight {
                    NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
                }

                VoiceOrb {
                    anchors.centerIn: parent
                    state: root.voiceState
                    size: 120
                    onClicked: {
                        // Toggle voice
                        if (root.voiceState !== "idle") root.voiceState = "idle"
                    }
                }
            }

            // === ChatView ===
            ChatView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                messages: root.mockMessages
                onSuggestionClicked: function(text) {
                    console.log("Sugerencia:", text)
                    // TODO: enviar a Rust backend
                }
                onImageClicked: function(url, caption) {
                    imagePreview.show(url, caption)
                }
            }

            // === Input bar (flotante) ===
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 64
                Layout.margins: Theme.spacingSm

                InputBar {
                    anchors.fill: parent
                    streaming: root.streaming
                    recording: root.voiceState === "listening"
                    onSendClicked: function(text) {
                        console.log("Enviar:", text)
                        root.streaming = true
                    }
                    onMicClicked: {
                        root.voiceState = root.voiceState === "listening" ? "idle" : "listening"
                    }
                    onStopClicked: {
                        root.streaming = false
                        root.voiceState = "idle"
                    }
                }
            }
        }
    }

    // === Image preview dialog (overlay) ===
    ImagePreviewDialog {
        id: imagePreview
        anchors.fill: parent
        open_: false
    }

    // === Settings dialog (overlay) ===
    SettingsDialog {
        id: settings
        anchors.fill: parent
        open_: false
        onClosed: console.log("Settings closed")
        onSaved: console.log("Settings saved")
    }

    // === Hotkey polling: lee ~/.cache/kde-assistant/hotkey.state ===
    // El backend Rust (rdev) escribe a este archivo cuando detecta
    // Super+Shift+A (toggle), Super+Shift+V (PTT), Ctrl+Shift+K (new session)
    property string hotkeyStamp: ""
    property string lastHotkeyStamp: ""
    property string homePath: {
        // Qt6: intentar Qt.platform.environment, fallback a HOME hardcodeada
        var env = Qt.platform.environment
        var home = env ? env["HOME"] : null
        if (!home) home = "/root"
        return "file://" + home + "/.cache/kde-assistant/hotkey.state"
    }
    Timer {
        id: hotkeyTimer
        interval: 300
        running: true
        repeat: true
        onTriggered: {
            var req = new XMLHttpRequest()
            req.open("GET", root.homePath + "?t=" + Date.now())
            req.onreadystatechange = function() {
                if (req.readyState === 4) {
                    if (req.status === 200 || req.status === 0) {
                        var content = req.responseText.trim()
                        if (content && content !== root.lastHotkeyStamp) {
                            root.lastHotkeyStamp = content
                            var parts = content.split("|")
                            var action = parts[0]
                            console.log("Hotkey recibido:", action)
                            if (action === "toggle_window") {
                                root.visible = !root.visible
                            } else if (action === "push_to_talk") {
                                root.voiceState = root.voiceState === "listening" ? "idle" : "listening"
                            } else if (action === "new_session") {
                                root.currentSessionId = ""
                            }
                        }
                    }
                }
            }
            req.send()
        }
    }
}
