// FileScene.qml - Escena de operaciones con archivos (crear, leer, editar).
// Secuencia: gaze zona archivos → thinking → inspect → success/happy

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
        { name: "gaze", dur: 400, fn: _phaseGaze },
        { name: "think", dur: 800, fn: _phaseThink },
        { name: "inspect", dur: 1000, fn: _phaseInspect },
        { name: "result", dur: 1200, fn: _phaseResult }
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
        if (gazeCtrl) gazeCtrl.lookAtZone("files", 1500)
        if (expressionCtrl) expressionCtrl.request("curious")
    }

    function _phaseThink() {
        if (expressionCtrl) expressionCtrl.request("thinking")
        if (moodCtrl) moodCtrl.setMood("focused", 0.8, 0)
        if (animationCtrl) animationCtrl.play("rock", 800)
    }

    function _phaseInspect() {
        if (expressionCtrl) expressionCtrl.flash("curious", 1000)
        if (gazeCtrl) gazeCtrl.lookAtZone("files", 1000)
    }

    function _phaseResult() {
        var success = true // params.success !== false
        if (success) {
            if (expressionCtrl) expressionCtrl.flash("happy", 1200)
            if (moodCtrl) moodCtrl.setMood("happy", 0.7, 10)
            if (animationCtrl) animationCtrl.play("bounce", 800)
        } else {
            if (expressionCtrl) expressionCtrl.flash("unsure", 1200)
            if (moodCtrl) moodCtrl.setMood("concerned", 0.8, 15)
        }
    }

    function _finish() { finished() }
    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
