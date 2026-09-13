// AgentWindow.qml - Contenedor del Assistant Character.
// Funciona tanto como Window standalone (XWayland, Fases 2-3)
// como Item dentro de AgentOverlay (Wayland nativo, Fase 4).

import QtQuick

Item {
    id: root

    property bool agentEnabled: true
    property bool reducedMotion: false
    property int characterSize: 140
    property string presenceMode: "companion"
    property int sleepAfterSecs: 240

    property string assistantState: "idle"
    property real voiceLevel: 0.0

    width: characterSize
    height: characterSize
    visible: agentEnabled

    AssistantCharacter {
        id: character
        anchors.centerIn: parent
        size: root.characterSize
        pose: exprCtrl.pose
        breatheAmp: moodCtrl.breatheAmp
    }

    ExpressionController { id: exprCtrl }

    AgentAnimationController {
        id: animCtrl
        character: character
        reducedMotion: root.reducedMotion
        breathePeriodMs: moodCtrl.breathePeriodMs
    }

    MoodController {
        id: moodCtrl
        expressionCtrl: exprCtrl
    }

    GazeController {
        id: gazeCtrl
        character: character
        window: root
    }

    IdleBehavior {
        id: idle
        gaze: gazeCtrl
        expressionCtrl: exprCtrl
        moodCtrl: moodCtrl
        character: character
        sleepAfterSecs: root.sleepAfterSecs
        onFellAsleep: console.log("agent: zzz (inactividad)")
        onWokeUp: console.log("agent: despierto")
    }

    MovementController {
        id: moveCtrl
        character: character
        groundY: root.height - character.height / 2 - 8
        _physicsTimer: physicsTickTimer
    }

    // El Timer del MovementController vive aquí (un Item es hijo válido)
    Timer {
        id: physicsTickTimer
        interval: 16
        repeat: true
        running: true
        onTriggered: moveCtrl._physicsTick()
    }

    SceneController {
        id: sceneCtrl
        characterCtrl: characterCtrl
        expressionCtrl: exprCtrl
        animationCtrl: animCtrl
        moodCtrl: moodCtrl
        gazeCtrl: gazeCtrl
        movementCtrl: moveCtrl
    }

    IntentRouter {
        id: intentRouter
        characterCtrl: characterCtrl
        sceneCtrl: sceneCtrl
    }

    VisualGuide {
        id: visualGuide
        enabled: (presenceMode === "companion" || presenceMode === "cinematic")
    }

    DebugPanel {
        id: debugPanel
        characterCtrl: characterCtrl
        expressionCtrl: exprCtrl
        animationCtrl: animCtrl
        moodCtrl: moodCtrl
        gazeCtrl: gazeCtrl
        idleBehavior: idle
        movementCtrl: moveCtrl
        sceneCtrl: sceneCtrl
        intentRouter: intentRouter
    }

    CharacterController {
        id: characterCtrl
        expressionCtrl: exprCtrl
        animationCtrl: animCtrl
        moodCtrl: moodCtrl
        gazeCtrl: gazeCtrl
        idleBehavior: idle
        movementCtrl: moveCtrl
        sceneCtrl: sceneCtrl
        intentRouter: intentRouter
        visualGuide: visualGuide
        busy: root.assistantState !== "idle"
    }

    x: moveCtrl.x
    y: moveCtrl.y

    onAssistantStateChanged: {
        if (assistantState === "idle") characterCtrl.handleEvent("idle", "NORMAL")
        else if (assistantState === "listening") characterCtrl.handleEvent("listening", "IMPORTANT")
        else if (assistantState === "processing") characterCtrl.handleEvent("processing", "NORMAL")
        else if (assistantState === "speaking") characterCtrl.handleEvent("speaking", "NORMAL")
    }

    onPresenceModeChanged: applyModeConstraints()
    function applyModeConstraints() {
        var m = presenceMode
        idle.enabled = (m !== "minimal")
        gazeCtrl.enabled = (m !== "minimal")
        visualGuide.enabled = (m === "companion" || m === "cinematic")
    }

    // Actualizar zonas de escritorio reales (Fase 7)
    function updateDesktopZones(zones) {
        gazeCtrl.desktopZones = zones
    }

    // Tecla de debug: Ctrl+Shift+D (solo en companion/cinematic)
    Keys.onPressed: {
        if (event.key === Qt.Key_D && (event.modifiers & Qt.ControlModifier) && (event.modifiers & Qt.ShiftModifier)) {
            if (presenceMode === "companion" || presenceMode === "cinematic") {
                debugPanel.toggle()
            }
        }
    }

    Component.onCompleted: {
        applyModeConstraints()
        moveCtrl.setHome(width/2, height - character.height/2 - 8)
        moveCtrl.appear(width/2, height - character.height/2 - 8)
        animCtrl.play("breathe")
        animCtrl.play("blink")
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        drag.target: undefined

        onPressed: function(mouse) {
            if (root.presenceMode === "minimal") return
            moveCtrl.startDrag(root.x + mouse.x, root.y + mouse.y)
            idle.notifyActivity("dragStart")
        }
        onPositionChanged: function(mouse) {
            if (root.presenceMode === "minimal") return
            if (moveCtrl.isDragging) {
                moveCtrl.updateDrag(root.x + mouse.x, root.y + mouse.y)
            } else {
                gazeCtrl.lookAtLocal(mouse.x, mouse.y, width, height)
                idle.notifyActivity("hover")
            }
        }
        onReleased: function(mouse) {
            if (root.presenceMode === "minimal") return
            if (moveCtrl.isDragging) {
                moveCtrl.endDrag(0, 0)
                idle.notifyActivity("dragEnd")
            }
        }
        onExited: { if (!moveCtrl.isDragging) gazeCtrl.rest(500) }
        onClicked: {
            if (moveCtrl.isDragging) return
            idle.notifyActivity("click")
            characterCtrl.handleEvent("wink", "NORMAL")
            root.clickedByUser()
        }
        onDoubleClicked: {
            if (moveCtrl.isDragging) return
            idle.notifyActivity("double-click")
            characterCtrl.handleEvent("happy", "NORMAL")
        }
    }

    signal clickedByUser()

    // DevPreview helpers
    function devWalkTo(x, y) { characterCtrl.handleEvent("walk", "NORMAL", {x: x, y: y}) }
    function devRunTo(x, y) { characterCtrl.handleEvent("run", "NORMAL", {x: x, y: y}) }
    function devJump() { characterCtrl.handleEvent("jump", "NORMAL") }
    function devBounce(intensity) { characterCtrl.handleEvent("bounce", "NORMAL", {intensity: intensity}) }
    function devReturn() { characterCtrl.handleEvent("return", "NORMAL") }
    function devAppear(x, y) { characterCtrl.handleEvent("appear", "NORMAL", {x: x, y: y}) }
    function devDisappear() { characterCtrl.handleEvent("disappear", "NORMAL") }

    function devSceneSearch() { sceneCtrl.play("search") }
    function devSceneOpenApp() { sceneCtrl.play("openapp") }
    function devSceneFile() { sceneCtrl.play("file") }
    function devSceneSystem() { sceneCtrl.play("system") }
    function devSceneError() { sceneCtrl.play("error") }
    function devSceneSuccess() { sceneCtrl.play("success") }
    function devSceneDownload() { sceneCtrl.play("download") }
    function devSceneScreenshot() { sceneCtrl.play("screenshot") }
    function devToolOpenApp() { characterCtrl.handleEvent("tool:open_app", "NORMAL") }
    function devToolFindFile() { characterCtrl.handleEvent("tool:find_file", "NORMAL") }
    function devToolWebSearch() { characterCtrl.handleEvent("tool:web_search", "NORMAL") }
    function devToolCreateFile() { characterCtrl.handleEvent("tool:create_file", "NORMAL") }
    function devToolError() { characterCtrl.handleEvent("tool:unknown_tool", "NORMAL") }

    // VisualGuide helpers
    function devGuideArrow(tx, ty) { visualGuide.showArrow(tx, ty, width/2, height/2, "Aquí") }
    function devGuideHighlight(x, y, w, h) { visualGuide.showHighlight(x, y, w, h, "Zona") }
    function devGuidePulse(x, y) { visualGuide.showPulse(x, y, "Punto") }
}
