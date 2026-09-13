// SearchScene.qml - Escena de búsqueda (web, archivos, etc.).
// Secuencia: thinking → gaze zona archivos → search anim → inspect → found → happy

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

    // Fases de la escena: cada una = {duración, función}
    readonly property var _phases: [
        { name: "think", dur: 800, fn: _phaseThink },
        { name: "gaze", dur: 600, fn: _phaseGaze },
        { name: "search", dur: 1500, fn: _phaseSearch },
        { name: "inspect", dur: 800, fn: _phaseInspect },
        { name: "found", dur: 1000, fn: _phaseFound },
        { name: "happy", dur: 1200, fn: _phaseHappy }
    ]

    signal started()
    signal finished()
    signal cancelled()

    function start(params) {
        _phaseIndex = 0
        _runPhase()
    }

    function _runPhase() {
        if (_phaseIndex >= _phases.length) {
            _finish()
            return
        }
        var phase = _phases[_phaseIndex]
        _phase = phase.name
        phase.fn()
        // Avanzar tras duración
        Qt.callLater(function() {
            var timer = Qt.createQmlObject('import QtQuick; Timer { interval: ' + phase.dur + '; repeat: false; running: true; onTriggered: _advancePhase() }', scene)
        })
    }

    function _advancePhase() {
        _phaseIndex++
        _runPhase()
    }

    function _phaseThink() {
        if (expressionCtrl) expressionCtrl.request("thinking")
        if (moodCtrl) moodCtrl.setMood("focused", 0.8, 0)
        if (gazeCtrl) gazeCtrl.lookAtZone("files", 2000)
    }

    function _phaseGaze() {
        // Mantener gaze en zona archivos
        if (gazeCtrl) gazeCtrl.lookAtZone("files", 1000)
    }

    function _phaseSearch() {
        if (expressionCtrl) expressionCtrl.flash("thinking", 1500)
        if (animationCtrl) animationCtrl.play("rock", 1500) // balanceo thinking
    }

    function _phaseInspect() {
        if (expressionCtrl) expressionCtrl.request("curious")
        if (gazeCtrl) gazeCtrl.lookAtZone("files", 800)
    }

    function _phaseFound() {
        if (expressionCtrl) expressionCtrl.flash("surprised", 600)
        if (animationCtrl) animationCtrl.play("bounce", 400)
    }

    function _phaseHappy() {
        if (expressionCtrl) expressionCtrl.flash("happy", 1200)
        if (moodCtrl) moodCtrl.setMood("happy", 0.7, 10)
        if (animationCtrl) animationCtrl.play("bounce", 600)
    }

    function _finish() {
        finished()
    }

    function cancel() {
        _phaseIndex = _phases.length // saltar a finish
        _finish()
        cancelled()
    }
}
