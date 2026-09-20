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
    property string characterAppearance: "capsule"
    property color characterAccentColor: "#0094bb"
    property string presenceMode: "companion"
    property int sleepAfterSecs: 240

    property string assistantState: "idle"
    // Solo wake word/PTT despiertan el personaje; escribir en el chat no
    // deja una mascota permanente sobre el escritorio.
    property bool invoked: false
    property real voiceLevel: 0.0

    title: "KDE Assistant Character"
    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    width: characterSize + 16
    height: characterSize + 16
    minimumWidth: characterSize + 16
    minimumHeight: characterSize + 16
    maximumWidth: characterSize + 16
    maximumHeight: characterSize + 16

    visible: agentEnabled && (invoked || agentWindow.activityVisible)
    function handleActivity(event) { agentWindow.handleActivity(event) }
    function resetActivity() { agentWindow.resetActivity() }

    // === AgentWindow (Item) centrado en la Window ===
    AgentWindow {
        id: agentWindow
        anchors.centerIn: parent
        agentEnabled: root.agentEnabled
        reducedMotion: root.reducedMotion
        characterSize: root.characterSize
        characterAppearance: root.characterAppearance
        characterAccentColor: root.characterAccentColor
        presenceMode: root.presenceMode
        sleepAfterSecs: root.sleepAfterSecs
        assistantState: root.assistantState
        voiceLevel: root.voiceLevel
        moveWholeWindow: true
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
        function onMoveRequested(dx, dy) {
            root.x = Math.max(0, Math.min(Screen.desktopAvailableWidth - root.width, root.x + dx))
            root.y = Math.max(0, Math.min(Screen.desktopAvailableHeight - root.height, root.y + dy))
        }
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
