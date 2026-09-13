// CharacterController.qml - Fachada del personaje: orquesta Expression,
// Animation, Mood, Gaze, Idle, Movement, Scenes, IntentRouter, VisualGuide
// y expone la API de alto nivel para la app.
//
// Fase 2: eventos -> mood (con dwell/ttl) + expresiones flash + mirada.
// Fase 3: eventos de movimiento (walk/run/jump/bounce/drag/return/appear/disappear)
// Fase 5: escenas coreografiadas (tool → scene mapping, prioridades, cancelación)
// Fase 6: IntentRouter (whitelist, visual intent, prioridades, cancelación end-to-end)
// Fase 7: VisualGuide (flechas/highlights/pulsos), zonas contextuales de escritorio
//
// Prioridades: CRITICAL > ERROR > IMPORTANT > NORMAL > IDLE.
// Un evento de prioridad menor se ignora (eventDropped); uno mayor lo desplaza.
// El idle autónomo NUNCA interrumpe (IdleBehavior se pausa con busy=true).

import QtQuick

QtObject {
    id: ctrl

    // Subsistemas
    property var expressionCtrl: null
    property var animationCtrl: null
    property var moodCtrl: null
    property var gazeCtrl: null
    property var idleBehavior: null
    property var movementCtrl: null
    property var sceneCtrl: null
    property var intentRouter: null
    property var visualGuide: null

    // Prioridad en curso
    property int _activePriority: 0

    readonly property var priorityLevels: ({
        "IDLE": 0, "NORMAL": 1, "IMPORTANT": 2, "SECURITY": 3, "ERROR": 4, "CRITICAL": 5
    })

    // Estado ocupado (pausa idle autónomo)
    property bool busy: false
    onBusyChanged: if (idleBehavior) idleBehavior.busy = busy

    signal eventHandled(string event, int priority)
    signal eventDropped(string event, int priority)
    signal movementEvent(string event, var params)
    signal sceneEvent(string event, var params)
    signal toolCallEvent(string tool, var params)

    // === Mapeo tool → escena ===
    readonly property var toolToScene: ({
        "open_app": "openapp", "create_file": "file", "edit_file": "file",
        "read_file": "file", "find_file": "search", "web_search": "search",
        "show_image": "success", "open_file": "file", "open_url": "openapp",
        "system_info": "system", "notify": "success", "media": "system",
        "volume": "system", "brightness": "system", "network_status": "system",
        "remind_in": "success", "kdeconnect": "system"
    })

    // === Eventos principales ===
    function handleEvent(kind, priorityName, params) {
        var prio = priorityLevels[priorityName] !== undefined ? priorityLevels[priorityName] : 0
        if (prio < _activePriority) {
            eventDropped(kind, prio)
            return false
        }
        _activePriority = prio
        eventHandled(kind, prio)

        if (idleBehavior) idleBehavior.notifyActivity(kind)

        // Tool call → IntentRouter (Fase 6)
        if (kind.startsWith("tool:")) {
            var toolName = kind.substring(5)
            if (intentRouter) {
                intentRouter.processToolCall(toolName, params?.toolCallId, params, params?.approvalNeeded)
            }
            toolCallEvent(toolName, params)
            return true
        }

        // Movimiento → MovementController
        if (movementCtrl && _isMovementEvent(kind)) {
            _handleMovementEvent(kind, params)
            return true
        }

        // VisualGuide commands (Fase 7)
        if (kind.startsWith("guide:")) {
            _handleGuideCommand(kind.substring(6), params)
            return true
        }

        // Eventos de estado del asistente
        switch (kind) {
        case "wake":
            if (expressionCtrl) expressionCtrl.flash("surprised", 650)
            if (gazeCtrl) gazeCtrl.rest(250)
            break
        case "listening":
            _setMood("curious", 0.8, 40)
            if (gazeCtrl) gazeCtrl.rest(180)
            break
        case "processing":
            _setMood("focused", 0.9, 90)
            break
        case "speaking":
            _setMood("happy", 0.7, 25)
            break
        case "done":
            if (expressionCtrl) expressionCtrl.flash("happy", 1400)
            if (moodCtrl) moodCtrl.setMood("happy", 0.7, 12)
            break
        case "error":
            if (expressionCtrl) expressionCtrl.flash("unsure", 1600)
            _setMood("concerned", 0.8, 45)
            if (sceneCtrl) sceneCtrl.play("error", params)
            break
        case "happy":
            if (expressionCtrl) expressionCtrl.flash("happy", 1200)
            break
        case "wink":
            if (expressionCtrl) expressionCtrl.flash("wink", 900)
            break
        case "curious":
            _setMood("curious", 0.8, 30)
            break
        case "focused":
            _setMood("focused", 0.9, 60)
            break
        case "idle":
            _setMood("neutral", 0.7, 0)
            if (gazeCtrl) gazeCtrl.rest(400)
            break
        case "sleep":
            if (idleBehavior) idleBehavior.sleep()
            break
        case "wakeIdle":
            if (idleBehavior) idleBehavior.wake()
            break
        default:
            _setMood("neutral", 0.7, 0)
        }
        return true
    }

    function _handleGuideCommand(cmd, params) {
        if (!visualGuide) return false
        switch (cmd) {
        case "arrow":
            visualGuide.showArrow(params.targetX, params.targetY, params.sourceX, params.sourceY, params.label)
            break
        case "highlight":
            visualGuide.showHighlight(params.x, params.y, params.width, params.height, params.label)
            break
        case "pulse":
            visualGuide.showPulse(params.x, params.y, params.label)
            break
        case "path":
            visualGuide.showPath(params.sourceX, params.sourceY, params.targetX, params.targetY)
            break
        case "hide":
            visualGuide.hide()
            break
        }
        return true
    }

    // Callback desde backend: resultado de tool call
    function onToolResult(toolCallId, success, result, imageUrl) {
        if (intentRouter) {
            intentRouter.onToolResult(toolCallId, success, result, imageUrl)
        }
    }

    // Callback: aprobación de tool sensible
    function onToolApproved(toolCallId, approved) {
        if (intentRouter && approved) {
            // El backend re-ejecutará el tool
        } else if (intentRouter && !approved) {
            if (sceneCtrl) sceneCtrl.play("error", { denied: true })
        }
    }

    // Actualizar zonas de escritorio (Fase 7)
    function updateDesktopZones(zones) {
        if (gazeCtrl) gazeCtrl.desktopZones = zones
    }

    function _isMovementEvent(kind) {
        return kind === "walk" || kind === "run" || kind === "jump" ||
               kind === "bounce" || kind === "dragStart" || kind === "dragEnd" ||
               kind === "return" || kind === "appear" || kind === "disappear"
    }

    function _handleMovementEvent(kind, params) {
        var handled = false
        switch (kind) {
        case "walk": handled = movementCtrl.walkTo(params.x, params.y); break
        case "run": handled = movementCtrl.runTo(params.x, params.y); break
        case "jump": handled = movementCtrl.jump(); break
        case "bounce": handled = movementCtrl.bounce(params.intensity); break
        case "dragStart": handled = movementCtrl.startDrag(params.x, params.y); break
        case "dragEnd": handled = movementCtrl.endDrag(params.throwVx, params.throwVy); break
        case "return": handled = movementCtrl.returnHome(); break
        case "appear": handled = movementCtrl.appear(params.x, params.y, params.fromDirection); break
        case "disappear": handled = movementCtrl.disappear(); break
        }
        if (handled) movementEvent(kind, params)
        return handled
    }

    function _setMood(name, intensity, ttlSecs) {
        if (moodCtrl) moodCtrl.setMood(name, intensity, ttlSecs)
        else if (expressionCtrl) {
            expressionCtrl.request(name === "curious" || name === "focused" ? "thinking" : name)
        }
    }

    function lookAtZone(name, holdMs) {
        if (gazeCtrl) gazeCtrl.lookAtZone(name, holdMs)
    }

    function release(priorityName) {
        var prio = priorityLevels[priorityName] !== undefined ? priorityLevels[priorityName] : 0
        if (_activePriority === prio) _activePriority = 0
    }

    function reset() {
        _activePriority = 0
        if (animationCtrl) animationCtrl.cancelAll()
        if (expressionCtrl) expressionCtrl.request(expressionCtrl.baseExpression)
        if (gazeCtrl) gazeCtrl.rest(0)
        if (movementCtrl) movementCtrl.cancel()
        if (sceneCtrl) sceneCtrl.cancel()
        if (intentRouter) intentRouter.reset()
        if (visualGuide) visualGuide.hide()
    }
}
