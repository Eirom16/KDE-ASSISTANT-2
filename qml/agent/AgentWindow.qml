// AgentWindow.qml - Contenedor del Assistant Character.
// Funciona tanto como Window standalone (XWayland, Fases 2-3)
// como Item dentro de AgentOverlay (Wayland nativo, Fase 4).

import QtQuick
import qml 1.0

Item {
    id: root

    property bool agentEnabled: true
    property bool reducedMotion: false
    property int characterSize: 140
    property string characterAppearance: "capsule"
    property color characterAccentColor: "#0094bb"
    property string presenceMode: "companion"
    property int sleepAfterSecs: 240
    property bool moveWholeWindow: true
    property bool hostVisible: true
    readonly property bool autonomousMotion: presenceMode === "companion" || presenceMode === "cinematic"
    property real _lastDragMouseX: 0
    property real _lastDragMouseY: 0

    property string assistantState: "idle"
    property real voiceLevel: 0.0
    readonly property bool activityVisible: sceneCtrl.active
    readonly property string currentScene: sceneCtrl._currentScene
    readonly property string scenePhase: sceneCtrl.phase
    readonly property string currentRunId: sceneCtrl.runId
    readonly property string currentToolCallId: sceneCtrl.toolCallId
    property var pendingActions: []
    function handleActivity(event) {
        if (!agentEnabled || !event) return
        if (event.type === "run") {
            if (event.phase === "cancelled" || event.phase === "error") {
                pendingActions = pendingActions.filter(function(a) { return a.run_id !== event.run_id })
                if (sceneCtrl.runId === event.run_id) sceneCtrl.cancel()
            }
            return
        }
        if (event.type !== "intent") return
        var id = event.tool_call_id
        var run = event.run_id || ""
        if (event.phase === "start" || event.phase === "approval") {
            if (sceneCtrl.toolCallId === id && sceneCtrl.runId === run) {
                sceneCtrl.phase = event.phase === "approval" ? "approval" : "working"
                sceneCtrl.animateWork()
            } else if (!sceneCtrl.active) startAction(event)
            else {
                var queue = pendingActions.slice()
                var found = false
                for (var i = 0; i < queue.length; i++) {
                    if (queue[i].tool_call_id === id && queue[i].run_id === run) { queue[i] = event; found = true; break }
                }
                if (!found) queue.push(event)
                pendingActions = queue.slice(-4)
            }
        } else {
            if (sceneCtrl.toolCallId === id && sceneCtrl.runId === run) sceneCtrl.result(id, event.phase, run)
            else {
                var queued = pendingActions.slice()
                var matched = false
                for (var j = 0; j < queued.length; j++) {
                    if (queued[j].tool_call_id === id && queued[j].run_id === run) {
                        queued[j].result = event.phase
                        matched = true
                    }
                }
                if (!matched && event.phase !== "done") {
                    var synthetic = {
                        type: event.type, run_id: run, tool_call_id: id,
                        tool: event.tool || "", phase: "approval", result: event.phase
                    }
                    if (!sceneCtrl.active) startAction(synthetic)
                    else queued.push(synthetic)
                }
                pendingActions = queued.slice(-4)
            }
        }
    }
    function startAction(event) {
        sceneCtrl.play(sceneCtrl.sceneForTool(event.tool), {
            tool: event.tool, toolCallId: event.tool_call_id, runId: event.run_id,
            approval: event.phase === "approval"
        })
        if (event.result) sceneCtrl.result(event.tool_call_id, event.result, event.run_id)
    }
    function resetActivity() { pendingActions = []; sceneCtrl.cancel() }

    function ensureVisible() {
        if (!character) return
        character.visible = true
        if (character.opacity < 0.2 || character.scale < 0.2) {
            character.opacity = 1.0
            character.scale = 1.0
            character.squashX = 1.0
            character.squashY = 1.0
        }
        animCtrl.play("breathe")
        animCtrl.play("blink")
    }

    function softDisappear() {
        if (!character) return
        if (reducedMotion) {
            character.visible = false
            return
        }
        animCtrl.play("disappear")
    }

    function _randomRange(minValue, maxValue) {
        return minValue + Math.random() * (maxValue - minValue)
    }

    function _scheduleNextLifeMove() {
        lifeMoveTimer.interval = Math.round(_randomRange(2600, 6200))
    }

    function _doAutonomousMove() {
        if (!agentEnabled || reducedMotion || !autonomousMotion || !hostVisible) return
        if (sceneCtrl.active || moveCtrl.isDragging) return

        var r = Math.random()
        if (r < 0.28) {
            characterCtrl.handleEvent("curious", "IDLE")
            return
        }
        if (r < 0.48) {
            moveCtrl.bounce(_randomRange(0.12, 0.32))
            return
        }

        var dx = _randomRange(-34, 34)
        var dy = _randomRange(-18, 18)
        if (Math.abs(dx) < 10) dx = dx < 0 ? -10 : 10
        if (moveWholeWindow) {
            moveRequested(dx, dy)
            moveCtrl.bounce(_randomRange(0.08, 0.18))
        } else {
            moveCtrl.walkTo(Math.max(character.width / 2, Math.min(width - character.width / 2, moveCtrl.x + dx)),
                            Math.max(character.height / 2, Math.min(height - character.height / 2, moveCtrl.y + dy)))
        }
    }

    width: characterSize
    height: characterSize
    visible: agentEnabled

    AssistantCharacter {
        id: character
        size: root.characterSize
        appearance: root.characterAppearance
        accentColor: root.characterAccentColor
        pose: exprCtrl.pose
        breatheAmp: moodCtrl.breatheAmp
        x: moveCtrl.x - width / 2
        y: moveCtrl.y - height / 2
    }

    // Local props communicate the kind of work even with reduced motion.
    Rectangle {
        visible: sceneCtrl.active
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        width: Math.min(parent.width, 150); height: 32
        radius: Theme.radiusMd; color: Theme.surface
        border.color: Theme.hairline
        Row {
            anchors.centerIn: parent; spacing: Theme.spacingXs
            Octicon {
                name: sceneCtrl.phase === "done" ? "check-16" : sceneCtrl.phase === "error" || sceneCtrl.phase === "denied" ? "x-16"
                    : sceneCtrl.phase === "approval" ? "eye-16" : sceneCtrl._currentScene === "search" ? "search-16"
                    : sceneCtrl.tool === "create_file" ? "file-added-16" : sceneCtrl.tool === "edit_file" ? "pencil-16"
                    : sceneCtrl._currentScene === "openapp" ? "rocket-16" : "file-16"
                size: 16; color: Theme.primary
            }
            Text {
                text: sceneCtrl.phase === "done" ? qsTr("Hecho") : sceneCtrl.phase === "error" ? qsTr("Error")
                    : sceneCtrl.phase === "denied" ? qsTr("Denegado") : sceneCtrl.phase === "approval" ? qsTr("Confirmar")
                    : sceneCtrl._currentScene === "search" ? qsTr("Buscando") : sceneCtrl.tool === "create_file" ? qsTr("Creando")
                    : sceneCtrl.tool === "edit_file" ? qsTr("Editando") : sceneCtrl._currentScene === "openapp" ? qsTr("Abriendo") : qsTr("Revisando")
                color: Theme.ink; font.pixelSize: Theme.fontSizeCaption
            }
        }
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

    Timer {
        id: lifeMoveTimer
        interval: 4200
        repeat: true
        running: root.agentEnabled && root.hostVisible && root.autonomousMotion && !root.reducedMotion
                 && !sceneCtrl.active && !moveCtrl.isDragging
        onTriggered: {
            root._doAutonomousMove()
            root._scheduleNextLifeMove()
        }
    }

    SceneController {
        id: sceneCtrl
        characterCtrl: characterCtrl
        expressionCtrl: exprCtrl
        animationCtrl: animCtrl
        moodCtrl: moodCtrl
        gazeCtrl: gazeCtrl
        movementCtrl: moveCtrl
        onSceneFinished: {
            Qt.callLater(function() {
                if (root.pendingActions.length && !sceneCtrl.active) {
                    var next = root.pendingActions[0]
                    root.pendingActions = root.pendingActions.slice(1)
                    root.startAction(next)
                }
            })
        }
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
        busy: root.assistantState !== "idle" || sceneCtrl.active
    }

    // The host owns positioning; movement never moves the entire overlay offscreen.

    onAssistantStateChanged: {
        characterCtrl.release("IMPORTANT")
        if (assistantState !== "idle") ensureVisible()
        if (sceneCtrl.active) return
        if (assistantState === "idle") characterCtrl.handleEvent("idle", "NORMAL")
        else if (assistantState === "wake_detected" || assistantState === "attentive") characterCtrl.handleEvent("wake", "IMPORTANT")
        else if (assistantState === "listening") characterCtrl.handleEvent("listening", "IMPORTANT")
        else if (assistantState === "thinking" || assistantState === "processing" || assistantState === "tool_executing") characterCtrl.handleEvent("processing", "NORMAL")
        else if (assistantState === "responding" || assistantState === "speaking") characterCtrl.handleEvent("speaking", "NORMAL")
        else if (assistantState === "interrupted") characterCtrl.handleEvent("listening", "NORMAL")
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
        moveCtrl.x = moveCtrl.homeX
        moveCtrl.y = moveCtrl.homeY
        root._scheduleNextLifeMove()
        animCtrl.play("appear")
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
            root._lastDragMouseX = mouse.x
            root._lastDragMouseY = mouse.y
            moveCtrl.startDrag(moveCtrl.x, moveCtrl.y)
            idle.notifyActivity("dragStart")
        }
        onPositionChanged: function(mouse) {
            if (root.presenceMode === "minimal") return
            if (moveCtrl.isDragging) {
                if (root.moveWholeWindow) {
                    var dx = mouse.x - root._lastDragMouseX
                    var dy = mouse.y - root._lastDragMouseY
                    if (Math.abs(dx) > 0.1 || Math.abs(dy) > 0.1)
                        root.moveRequested(dx, dy)
                } else {
                    moveCtrl.updateDrag(mouse.x, mouse.y)
                }
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
    signal moveRequested(real dx, real dy)

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
