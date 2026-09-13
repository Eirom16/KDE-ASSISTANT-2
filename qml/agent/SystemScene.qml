// SystemScene.qml - Escena de operaciones del sistema (media, volumen, brillo, red).
// Secuencia: gaze zona notificaciones/media → acción → confirmación

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
        { name: "action", dur: 800, fn: _phaseAction },
        { name: "confirm", dur: 1000, fn: _phaseConfirm }
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
        // Mirar zona notificaciones (arriba derecha) o media (izquierda)
        var zone = (params && params.zone) ? params.zone : "notifications"
        if (gazeCtrl) gazeCtrl.lookAtZone(zone, 1500)
        if (expressionCtrl) expressionCtrl.request("curious")
        if (moodCtrl) moodCtrl.setMood("focused", 0.6, 0)
    }

    function _phaseAction() {
        if (expressionCtrl) expressionCtrl.flash("wink", 600)
        if (animationCtrl) animationCtrl.play("bounce", 500)
    }

    function _phaseConfirm() {
        if (expressionCtrl) expressionCtrl.flash("happy", 1000)
        if (moodCtrl) moodCtrl.setMood("happy", 0.7, 8)
    }

    function _finish() { finished() }
    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
