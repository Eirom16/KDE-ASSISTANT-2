// AgentWindowStandalone.qml - Wrapper Window para XWayland (Fases 2-3).
// Envuelve AgentWindow (Item) en una Window con flags Qt.Tool.
// En Fase 4, AgentMain.qml usa AgentWindow directamente como Item.

import QtQuick
import QtQuick.Window

Window {
    id: root

    // === Config (cfg.character.* via Main.qml) ===
    property bool agentEnabled: true
    property bool reducedMotion: false
    property int characterSize: 140
    property string presenceMode: "companion"
    property int sleepAfterSecs: 240

    property string assistantState: "idle"
    property real voiceLevel: 0.0

    title: "KDE Assistant Character"
    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    width: characterSize + 16
    height: characterSize + 16
    minimumWidth: width
    minimumHeight: height
    maximumWidth: width
    maximumHeight: height

    visible: agentEnabled

    // === AgentWindow (Item) centrado en la Window ===
    AgentWindow {
        id: agentWindow
        anchors.centerIn: parent
        agentEnabled: root.agentEnabled
        reducedMotion: root.reducedMotion
        characterSize: root.characterSize
        presenceMode: root.presenceMode
        sleepAfterSecs: root.sleepAfterSecs
        assistantState: root.assistantState
        voiceLevel: root.voiceLevel
    }

    // === Posicionamiento inicial (abajo-derecha) ===
    Component.onCompleted: {
        x = Screen.desktopAvailableWidth - width - 24
        y = Screen.desktopAvailableHeight - height - 20
    }

    // Reenviar señal clickedByUser
    Connections {
        target: agentWindow
        function onClickedByUser() { root.clickedByUser() }
    }

    signal clickedByUser()

    // DevPreview helpers (delegados a agentWindow)
    function devWalkTo(x, y) { agentWindow.devWalkTo(x, y) }
    function devRunTo(x, y) { agentWindow.devRunTo(x, y) }
    function devJump() { agentWindow.devJump() }
    function devBounce(intensity) { agentWindow.devBounce(intensity) }
    function devReturn() { agentWindow.devReturn() }
    function devAppear(x, y) { agentWindow.devAppear(x, y) }
    function devDisappear() { agentWindow.devDisappear() }
}
