// ExpressionController.qml - Control central de expresiones del personaje.
//
// Contrato (prompt maestro §3):
// - Cambio automatico por eventos O manual (request/force).
// - Anti-flap: una expresion recien fijada no se reemplaza antes de
//   minHoldMs; la peticion queda pendiente (la ultima gana) y se aplica
//   al cumplirse el hold.
// - Transiciones suaves: TODAS las expresiones son poses de 13 canales y se
//   interpolan linealmente (sin cortes visuales). Adoptar = rapido, volver a
//   la base = mas lento.
// - Expresiones temporales (durationMs): vuelven a baseExpression solas.

import QtQuick
import "CharacterData.js" as CharacterData

QtObject {
    id: ctrl

    // Expresion visible actual y base (la del mood, F2)
    property string expression: "idle"
    property string baseExpression: "idle"

    // Anti-flap
    property int minHoldMs: 900

    // Duracion del morph: al adoptar (rapido) y al volver a base (lento)
    property int morphInMs: 260
    property int morphOutMs: 480

    // Salida: pose mezclada lista para AssistantCharacter.pose
    readonly property var pose: CharacterData.lerpPose(_fromPose, _toPose, _blend)

    signal invalidExpression(string name)

    // --- interno ---
    property var _fromPose: CharacterData.poseOf("idle")
    property var _toPose: CharacterData.poseOf("idle")
    property real _blend: 1.0
    property string _pending: ""
    property double _lastChange: 0

    // Peticion normal: respeta anti-flap. Devuelve si fue aceptada como valida.
    function request(name) {
        if (!CharacterData.isValidExpression(name)) {
            console.warn("ExpressionController: expresion desconocida '" + name + "'")
            invalidExpression(name)
            return false
        }
        if (name === expression) { _pending = ""; return true }
        var now = Date.now()
        var elapsed = now - _lastChange
        if (elapsed < minHoldMs) {
            _pending = name
            holdTimer.interval = minHoldMs - elapsed
            holdTimer.restart()
            return true
        }
        _apply(name)
        return true
    }

    // Peticion con caducidad: muestra la expresion durationMs y vuelve a base.
    function flash(name, durationMs) {
        if (!request(name)) return
        if (durationMs > 0) {
            revertTimer.interval = durationMs
            revertTimer.restart()
        }
    }

    // Forzado (debug): salta el anti-flap.
    function force(name) {
        if (!CharacterData.isValidExpression(name)) return false
        holdTimer.stop()
        _pending = ""
        _apply(name)
        return true
    }

    function _apply(name) {
        var returningToBase = (name === baseExpression)
        // El origen del morph es la pose ACTUAL (a medio camino si habia una)
        _fromPose = CharacterData.lerpPose(_fromPose, _toPose, _blend)
        _toPose = CharacterData.poseOf(name)
        morphAnim.duration = returningToBase ? morphOutMs : morphInMs
        morphAnim.restart()
        expression = name
        // (expressionChanged llega via la signal automatica de la property)
        _lastChange = Date.now()
    }

    property Timer _holdTimer: Timer {
        id: holdTimer
        repeat: false
        onTriggered: {
            if (ctrl._pending !== "") {
                var p = ctrl._pending
                ctrl._pending = ""
                ctrl._apply(p)
            }
        }
    }

    property Timer _revertTimer: Timer {
        id: revertTimer
        repeat: false
        onTriggered: ctrl.request(ctrl.baseExpression)
    }

    property NumberAnimation _morph: NumberAnimation {
        id: morphAnim
        target: ctrl
        property: "_blend"
        from: 0.0
        to: 1.0
        easing.type: Easing.InOutQuad
        // duration la fija _apply (morphInMs/morphOutMs)
    }

    onBaseExpressionChanged: {
        // Cambio de base (mood): si no hay una expresion temporal en curso,
        // seguir a la nueva base. Si la hay, revertTimer pedira la nueva base
        // al terminar (lee baseExpression en el momento del disparo).
        if (!revertTimer.running) request(baseExpression)
    }
}
