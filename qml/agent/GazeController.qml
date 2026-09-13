// GazeController.qml - Sistema de mirada del personaje (prompt maestro §5).
//
// - Mirada con INERCIA: la duracion depende de la distancia (sacada larga =
//   salto rapido con overshoot + asentamiento; corta = deriva suave).
// - Micro-saccades ociosas (cada ~2.6-5.2s, +-0.35u, 140ms): los ojos no se
//   quedan "enclavados" (robotico) ni se mueven sin parar (molesto).
// - API de objetivos: puntos viewBox (lookAt), puntos de PANTALLA
//   (lookAtScreenPoint, requiere window) y zonas con nombre (lookAtZone).
// - Descanso: sin holdMs, vuelve a rest tras restDelayMs.
//
// Fase 7: zonas contextuales reales (pantalla completa) para overlay Wayland.
// Limitacion Wayland (DESKTOP-AGENT-DESIGN.md): la posicion global del
// cursor no es accesible; la mirada al cursor usa hover sobre la ventana y
// zonas aproximadas. En overlay nativo, lookAtScreenPoint usa coords globales.

import QtQuick

QtObject {
    id: gaze

    property Item character: null
    property var window: null

    property bool enabled: true
    property real travel: 3.0
    property int restDelayMs: 2400
    property point restPoint: Qt.point(0, 0.35)
    property bool saccadesEnabled: true

    // Fase 7: zonas de escritorio reales (coords globales -> dirección)
    // Se actualizan desde AgentOverlay/CharacterController
    property var desktopZones: ({})

    readonly property point current: Qt.point(curX, curY)
    readonly property bool moving: isMoving

    property real curX: 0
    property real curY: 0
    property point sacc: Qt.point(0, 0)
    property bool isMoving: false
    property int _holdMs: 0
    property string _zone: ""

    signal gazeSettled(point at)

    // ================= API =================

    function lookAt(x, y, holdMs) {
        if (!enabled) return
        _holdMs = holdMs && holdMs > 0 ? holdMs : 0
        _zone = ""
        _setTarget(Qt.point(x, y))
    }

    function lookAtScreenPoint(px, py, holdMs) {
        if (!enabled || !window) return
        var cx = window.x + window.width / 2
        var cy = window.y + window.height / 2
        var dx = px - cx
        var dy = py - cy
        var len = Math.sqrt(dx * dx + dy * dy)
        if (len < 1) { rest(); return }
        lookAt(dx / len * travel, dy / len * travel, holdMs)
    }

    function lookAtLocal(lx, ly, w, h) {
        if (!enabled) return
        var dx = (lx - w / 2) / (w / 2)
        var dy = (ly - h / 2) / (h / 2)
        _holdMs = 0
        _zone = ""
        restTimer.stop()
        _setTarget(Qt.point(dx * travel, dy * travel))
    }

    // Zonas con nombre + soporte para zonas de escritorio reales (Fase 7)
    function lookAtZone(name, holdMs) {
        // Primero buscar en desktopZones (posiciones reales de pantalla)
        if (desktopZones && desktopZones[name] !== undefined) {
            var dz = desktopZones[name]
            return lookAtScreenPoint(dz.x, dz.y, holdMs)
        }
        // Fallback: zonas direccionales relativas
        var zones = {
            "user": restPoint,
            "left": Qt.point(-travel, 0.4),
            "right": Qt.point(travel, 0.4),
            "up": Qt.point(0, -travel),
            "down": Qt.point(0, travel * 0.8),
            "top-right": Qt.point(travel * 0.8, -travel * 0.7),
            "top-left": Qt.point(-travel * 0.8, -travel * 0.7),
            "files": Qt.point(travel, 0.6),
            "apps": Qt.point(-travel, 0.6),
            "media": Qt.point(-travel * 0.5, 0.2),
            "notifications": Qt.point(travel * 0.8, -travel * 0.5),
            "center": restPoint
        }
        var z = zones[name] !== undefined ? zones[name] : restPoint
        _zone = name
        _setTarget(z)
        _holdMs = holdMs && holdMs > 0 ? holdMs : 30000
    }

    function glanceRandom() {
        if (!enabled) return
        var ang = Math.random() * Math.PI * 2
        var rad = 1.2 + Math.random() * 1.4
        _zone = ""
        _setTarget(Qt.point(Math.cos(ang) * rad, Math.sin(ang) * rad * 0.7))
        restTimer.interval = 900 + Math.random() * 900
        restTimer.restart()
    }

    function rest(delayMs) {
        _zone = ""
        restTimer.interval = delayMs !== undefined && delayMs >= 0 ? delayMs : 350
        restTimer.restart()
    }

    // ================= Interno =================

    function _setTarget(p) {
        var dist = Math.sqrt((curX - p.x) * (curX - p.x) + (curY - p.y) * (curY - p.y))
        driftAnim.stop(); saccadeMove.stop()
        isMoving = true
        if (dist > 1.9) {
            saccadeMove.toX = p.x; saccadeMove.toY = p.y
            saccadeMove.overX = curX + (p.x - curX) * 1.14
            saccadeMove.overY = curY + (p.y - curY) * 1.14
            saccadeMove.restart()
        } else {
            driftAnim.toX = p.x; driftAnim.toY = p.y
            driftAnim.dur = Math.min(430, Math.max(130, 130 + dist * 120))
            driftAnim.restart()
        }
        _scheduleRest()
    }

    function _scheduleRest() {
        if (_holdMs > 0) {
            restTimer.interval = _holdMs
            restTimer.restart()
        } else if (_zone === "" && !restTimer.running) {
            restTimer.interval = restDelayMs
            restTimer.restart()
        }
    }

    function _apply() {
        if (!character) return
        character.gazeOffset = character.clampGaze(Qt.point(curX + sacc.x, curY + sacc.y))
    }
    onCurXChanged: _apply()
    onCurYChanged: _apply()
    onSaccChanged: _apply()

    property SequentialAnimation _drift: SequentialAnimation {
        id: driftAnim
        property real toX: 0
        property real toY: 0
        property real dur: 260
        NumberAnimation { target: gaze; property: "curX"; to: driftAnim.toX; duration: driftAnim.dur; easing.type: Easing.OutQuad }
        NumberAnimation { target: gaze; property: "curY"; to: driftAnim.toY; duration: driftAnim.dur; easing.type: Easing.OutQuad }
        onFinished: gaze.isMoving = false
    }

    property SequentialAnimation _saccade: SequentialAnimation {
        id: saccadeMove
        property real toX: 0
        property real toY: 0
        property real overX: 0
        property real overY: 0
        ParallelAnimation {
            NumberAnimation { target: gaze; property: "curX"; to: saccadeMove.overX; duration: 90; easing.type: Easing.OutQuad }
            NumberAnimation { target: gaze; property: "curY"; to: saccadeMove.overY; duration: 90; easing.type: Easing.OutQuad }
        }
        ParallelAnimation {
            NumberAnimation { target: gaze; property: "curX"; to: saccadeMove.toX; duration: 170; easing.type: Easing.OutCubic }
            NumberAnimation { target: gaze; property: "curY"; to: saccadeMove.toY; duration: 170; easing.type: Easing.OutCubic }
        }
        onFinished: gaze.isMoving = false
    }

    property Timer _rest: Timer {
        id: restTimer
        repeat: false
        onTriggered: {
            gaze._holdMs = 0
            gaze._setTarget(gaze.restPoint)
        }
    }

    property Timer _saccTimer: Timer {
        id: saccadeTimer
        repeat: true
        interval: 3800
        running: true
        onTriggered: {
            interval = 2600 + Math.random() * 2600
            if (gaze.isMoving || !gaze.saccadesEnabled || !gaze.enabled) return
            gaze.sacc = Qt.point((Math.random() - 0.5) * 0.7, (Math.random() - 0.5) * 0.5)
            saccadeBack.restart()
        }
    }

    property Timer _saccBack: Timer {
        id: saccadeBack
        interval: 140
        repeat: false
        onTriggered: gaze.sacc = Qt.point(0, 0)
    }
}
