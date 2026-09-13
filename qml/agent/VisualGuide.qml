// VisualGuide.qml - Sistema de guía visual: flechas, highlights, indicadores
// para dirigir la atención del usuario hacia zonas/elementos del escritorio.
//
// Fase 7: overlay visual no intrusivo que acompaña al personaje.

import QtQuick
import QtQuick.Shapes

Item {
    id: guide

    property bool guideEnabled: true
    property var targetZone: null      // {x, y, width, height, name}
    property string guideType: "arrow" // "arrow" | "highlight" | "pulse" | "path"
    property real guideOpacity: 0.0
    property int duration: 3000        // ms auto-hide

    // Flecha direccional
    function showArrow(targetX, targetY, sourceX, sourceY, label) {
        if (!guideEnabled) return
        guideType = "arrow"
        _showArrow(targetX, targetY, sourceX, sourceY, label)
    }

    // Highlight rectangular (zona)
    function showHighlight(x, y, width, height, label) {
        if (!guideEnabled) return
        guideType = "highlight"
        targetZone = {x: x, y: y, width: width, height: height, label: label}
        _animateIn()
    }

    // Pulso en punto
    function showPulse(x, y, label) {
        if (!guideEnabled) return
        guideType = "pulse"
        targetZone = {x: x, y: y, width: 40, height: 40, label: label}
        _animatePulse()
    }

    // Camino/animación de atención (puntos que se mueven hacia target)
    function showPath(sourceX, sourceY, targetX, targetY) {
        if (!guideEnabled) return
        guideType = "path"
        _animatePath(sourceX, sourceY, targetX, targetY)
    }

    function hide() {
        _animateOut()
    }

    // === Implementación interna ===

    // Flecha SVG simple
    function _showArrow(tx, ty, sx, sy, label) {
        var dx = tx - sx
        var dy = ty - sy
        var angle = Math.atan2(dy, dx) * 180 / Math.PI
        var dist = Math.sqrt(dx*dx + dy*dy)

        // Crear flecha dinámicamente
        var arrow = Qt.createQmlObject(
            'import QtQuick; import QtQuick.Shapes; Item { ' +
            'property real angle: ' + angle + '; ' +
            'property real dist: ' + dist + '; ' +
            'width: 60; height: 60; ' +
            'transform: Rotation { origin.x: 30; origin.y: 30; angle: angle } ' +
            'Shape { anchors.centerIn: parent; width: 60; height: 60; ' +
            'ShapePath { fillColor: "#0094bb"; strokeColor: "transparent"; ' +
            'PathSvg { path: "M30 0 L10 20 L22 20 L22 40 L38 40 L38 20 L50 20 Z" } } } ' +
            'Text { anchors.top: parent.bottom; anchors.topMargin: 4; ' +
            'anchors.horizontalCenter: parent.horizontalCenter; ' +
            'text: "' + (label || "") + '"; font.pixelSize: 11; color: "#0094bb"; } }',
            guide, "arrowGuide")

        if (arrow) {
            arrow.x = sx - 30
            arrow.y = sy - 30
            _animateIn(arrow)
        }
    }

    function _animateIn(item) {
        var target = item || guide
        target.opacity = 0
        var anim = Qt.createQmlObject(
            'import QtQuick; SequentialAnimation { ' +
            'PropertyAnimation { target: target; property: "opacity"; from: 0; to: 0.9; duration: 200 } ' +
            'PauseAnimation { duration: ' + (guide.duration || 3000) + ' } ' +
            'PropertyAnimation { target: target; property: "opacity"; from: 0.9; to: 0; duration: 300 } ' +
            'ScriptAction { script: if (target !== guide) target.destroy() } }',
            guide, "fadeAnim")
        if (anim) anim.start()
    }

    function _animatePulse() {
        if (!targetZone) return
        var pulse = Qt.createQmlObject(
            'import QtQuick; Item { ' +
            'width: 40; height: 40; ' +
            'Rectangle { anchors.fill: parent; radius: 20; color: "#0094bb"; opacity: 0.6 } ' +
            'Text { anchors.centerIn: parent; text: "' + (targetZone.label || "") + '"; font.pixelSize: 10; color: "white" } }',
            guide, "pulseGuide")
        if (pulse) {
            pulse.x = targetZone.x - 20
            pulse.y = targetZone.y - 20
            var anim = Qt.createQmlObject(
                'import QtQuick; SequentialAnimation { ' +
                'PropertyAnimation { target: pulse; property: "scale"; from: 0.5; to: 1.5; duration: 600; easing.type: Easing.OutQuad } ' +
                'PropertyAnimation { target: pulse; property: "opacity"; from: 1; to: 0; duration: 600 } ' +
                'ScriptAction { script: pulse.destroy() } }',
                guide)
            if (anim) anim.start()
        }
    }

    function _animatePath(sx, sy, tx, ty) {
        // Puntos que se mueven del source al target
        var path = Qt.createQmlObject(
            'import QtQuick; Item { property real progress: 0 }',
            guide, "pathGuide")
        if (!path) return

        for (var i = 0; i < 5; i++) {
            var dot = Qt.createQmlObject(
                'import QtQuick; Rectangle { width: 8; height: 8; radius: 4; color: "#0094bb"; opacity: 0 }',
                path, "dot" + i)
            if (dot) {
                var delay = i * 100
                var anim = Qt.createQmlObject(
                    'import QtQuick; SequentialAnimation { ' +
                    'PauseAnimation { duration: ' + delay + ' } ' +
                    'PropertyAnimation { target: dot; property: "x"; from: ' + sx + '; to: ' + tx + '; duration: 800; easing.type: Easing.OutQuad } ' +
                    'PropertyAnimation { target: dot; property: "y"; from: ' + sy + '; to: ' + ty + '; duration: 800; easing.type: Easing.OutQuad } ' +
                    'PropertyAnimation { target: dot; property: "opacity"; from: 0; to: 0.8; duration: 200 } ' +
                    'PauseAnimation { duration: 300 } ' +
                    'PropertyAnimation { target: dot; property: "opacity"; from: 0.8; to: 0; duration: 200 } ' +
                    'ScriptAction { script: dot.destroy() } }',
                    path)
                if (anim) anim.start()
            }
        }
    }

    function _animateOut() {
        // Fade out any remaining children
        for (var i = 0; i < guide.children.length; i++) {
            var child = guide.children[i]
            if (child.opacity !== undefined) {
                var anim = Qt.createQmlObject(
                    'import QtQuick; PropertyAnimation { target: child; property: "opacity"; from: child.opacity; to: 0; duration: 200 }',
                    guide)
                if (anim) anim.start()
            }
        }
    }
}
