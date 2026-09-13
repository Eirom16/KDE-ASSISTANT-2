// OpenAppScene.qml - Escena de apertura de aplicación.
// Secuencia: gaze objetivo → walk/run hacia zona apps → bounce (click) → happy → return

import QtQuick

QtObject {
    id: scene

    property var characterCtrl: null
    property var expressionCtrl: null
    property var animationCtrl: null
    property var moodCtrl: null
    property var gazeCtrl: null
    property var movementCtrl: null

    property string _phase: "idle"
    property int _phaseIndex: 0

    readonly property var _phases: [
        { name: "gaze", dur: 500, fn: _phaseGaze },
        { name: "move", dur: 2000, fn: _phaseMove },
        { name: "action", dur: 600, fn: _phaseAction },
        { name: "celebrate", dur: 1000, fn: _phaseCelebrate },
        { name: "return", dur: 1500, fn: _phaseReturn }
    ]

    signal started()
    signal finished()
    signal cancelled()

    function start(params) {
        _phaseIndex = 0
        _runPhase()
    }

    function _runPhase() {
        if (_phaseIndex >= _phases.length) { _finish(); return }
        var phase = _phases[_phaseIndex]
        _phase = phase.name
        phase.fn()
        Qt.callLater(function() {
            var timer = Qt.createQmlObject('import QtQuick; Timer { interval: ' + phase.dur + '; repeat: false; running: true; onTriggered: _advancePhase() }', scene)
        })
    }

    function _advancePhase() { _phaseIndex++; _runPhase() }

    function _phaseGaze() {
        if (expressionCtrl) expressionCtrl.request("curious")
        if (gazeCtrl) gazeCtrl.lookAtZone("apps", 2000)
        if (moodCtrl) moodCtrl.setMood("curious", 0.8, 0)
    }

    function _phaseMove() {
        // Mover hacia zona apps (esquina inferior izquierda aprox)
        if (movementCtrl) {
            var targetX = 100  // zona apps izquierda
            var targetY = movementCtrl.groundY
            movementCtrl.runTo(targetX, targetY)
        }
    }

    function _phaseAction() {
        if (expressionCtrl) expressionCtrl.flash("surprised", 400)
        if (animationCtrl) animationCtrl.play("bounce", 600)
        if (movementCtrl) movementCtrl.bounce(0.8)
    }

    function _phaseCelebrate() {
        if (expressionCtrl) expressionCtrl.flash("happy", 1000)
        if (moodCtrl) moodCtrl.setMood("happy", 0.8, 8)
        if (animationCtrl) animationCtrl.play("bounce", 800)
    }

    function _phaseReturn() {
        if (movementCtrl) movementCtrl.returnHome()
        if (gazeCtrl) gazeCtrl.rest(500)
    }

    function _finish() { finished() }

    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
