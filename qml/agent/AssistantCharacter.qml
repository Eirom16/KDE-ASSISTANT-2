// AssistantCharacter.qml - Personaje del agente (identidad blobatar MIT,
// ver assets/character/ATTRIBUTION.md). Render 100% QML puro desde los
// datos numericos de CharacterData.js: cuerpo en estadio + 2 ojos como
// paths SVG absolutos, poses de 13 canales interpolables.
//
// Jerarquia de transforms (replica la composicion SVG de blobatar):
//   characterRoot (posicion en pantalla, squash de movimiento)
//   └── poseRoot     bdy + rock + shake + breathe (gesto global)
//       ├── body     estadio (Rectangle, radius = h/2)
//       └── gazeRoot offset de mirada (uniforme a ambos ojos)
//           └── eyeN T(centro) · R(rotPose) · S(sx, sy*blink) · R(-rotBase)
//
// API publica:
//   expression (string, via setExpression), pose (var, lectura),
//   gazeOffset (point en unidades viewBox, clamp ±3), size (px).
//   blinkScale/breathePhase los conduce AnimationController.

import QtQuick
import QtQuick.Shapes
import "CharacterData.js" as CharacterData

Item {
    id: chara

    property int size: 140
    width: size
    height: size
    property string appearance: "capsule"
    property color accentColor: CharacterData.body.fill

    // === Estado de pose (lo conduce ExpressionController) ===
    property var pose: CharacterData.poseOf("idle")
    readonly property string expression: _expression
    property string _expression: "idle"

    // === Mirada: offset en unidades viewBox (0-100), clamp +-3 ===
    // (blobatar gaze.css usa travel 3px sobre viewBox 100: mismo criterio)
    property point gazeOffset: Qt.point(0, 0)
    function clampGaze(p) {
        var limit = 3.0
        var len = Math.sqrt(p.x * p.x + p.y * p.y)
        if (len > limit) {
            return Qt.point(p.x * limit / len, p.y * limit / len)
        }
        return p
    }
    onGazeOffsetChanged: {
        var c = clampGaze(gazeOffset)
        if (c !== gazeOffset) gazeOffset = c
    }

    // === Canales "fisicos" (conducidos por AnimationController) ===
    property real blinkScale: 1.0      // 1 = ojo abierto, ~0.05 parpadeo
    property real breathePhase: 0.0    // 0..1 progreso de respiracion
    property real breatheAmp: 1.0      // amplitud (la fija el mood)
    property real squashX: 1.0         // squash & stretch (movimiento, F3)
    property real squashY: 1.0
    // Postura ociosa (IdleBehavior): leve rotacion de cuerpo en grados
    property real postureRot: 0.0
    property real actionX: 0.0
    property real actionY: 0.0
    property real actionRotation: 0.0
    // Dormido: Zzz + (el pose 'sleepy' lo fija el Mood/Idle controller)
    property bool sleeping: false

    // Unidad: px por unidad viewBox
    readonly property real u: size / CharacterData.VIEW_BOX

    // Centro del cuerpo (viewBox): item anclado ahi para rock/shake
    readonly property real bodyCx: (CharacterData.body.left + CharacterData.body.right) / 2.0
    readonly property real bodyCy: CharacterData.body.capY
    readonly property real baseBodyW: (CharacterData.body.right - CharacterData.body.left + 2 * CharacterData.body.r)
    readonly property real baseBodyH: (CharacterData.body.bottom - CharacterData.body.top)
    readonly property real bodyScaleX: appearance === "round" ? baseBodyH / baseBodyW
        : appearance === "pebble" ? 1.18
        : appearance === "compact" ? 0.82
        : 1.0
    readonly property real bodyScaleY: appearance === "pebble" ? 0.92
        : appearance === "compact" ? 1.04
        : 1.0
    readonly property real bodyVisualW: baseBodyW * bodyScaleX
    readonly property real bodyVisualH: baseBodyH * bodyScaleY
    readonly property real bodyRadiusFactor: appearance === "compact" ? 0.36 : 0.5
    readonly property color effectiveBodyColor: {
        if (pose.hot) return CharacterData.bodyColor(pose)
        return accentColor
    }

    // =====================================================================
    // Pose root: bdy (salto/gravedad de la pose) + rock + shake + breathe
    // =====================================================================
    Item {
        id: poseRoot
        width: chara.width
        height: chara.height

        // bdy en viewBox -> px; breathe estira un 2.5% maximo desde abajo
        x: chara.actionX
        y: chara.pose.bdy * chara.u + chara.actionY
        rotation: chara.actionRotation
        transform: [
            // Respiracion: escala Y anclada abajo (los "pies" no flotan);
            // amplitud modulada por el mood (breatheAmp).
            Scale {
                origin.x: chara.width / 2
                origin.y: chara.height * (CharacterData.body.bottom / CharacterData.VIEW_BOX)
                xScale: chara.squashX * (1.0 - chara.breathePhase * 0.014 * chara.breatheAmp)
                yScale: chara.squashY * (1.0 + chara.breathePhase * 0.028 * chara.breatheAmp)
            },
            // Postura ociosa (IdleBehavior): inclinacion leve del conjunto
            Rotation {
                origin.x: chara.bodyCx * chara.u
                origin.y: chara.height * (CharacterData.body.bottom / CharacterData.VIEW_BOX)
                angle: chara.postureRot
            },
            // Rock (p.ej. thinking=0.8): balanceo suave sobre el centro del cuerpo
            Rotation {
                origin.x: chara.bodyCx * chara.u
                origin.y: chara.bodyCy * chara.u
                angle: rockAnimator.angle
            },
            // Shake (mad/scared/sick): jitter horizontal de amplitud pose.shake
            Translate { x: shakeAnimator.offset }
        ]

        // === Cuerpo: estadio exacto (rect + caps), color con tinte por pose ===
        Rectangle {
            id: body
            x: (chara.bodyCx - chara.bodyVisualW / 2) * chara.u
            y: (chara.bodyCy - chara.bodyVisualH / 2) * chara.u
            width: chara.bodyVisualW * chara.u
            height: chara.bodyVisualH * chara.u
            radius: height * chara.bodyRadiusFactor
            color: chara.effectiveBodyColor
            antialiasing: true

            Behavior on color { ColorAnimation { duration: 320 } }
            Behavior on width { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
            Behavior on height { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
            Behavior on radius { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
        }

        // === Cara: 2 ojos (paths absolutos) bajo el offset de mirada ===
        Item {
            id: gazeRoot
            x: chara.gazeOffset.x * chara.u
            y: chara.gazeOffset.y * chara.u
            Behavior on x { NumberAnimation { duration: 130; easing.type: Easing.OutQuad } }
            Behavior on y { NumberAnimation { duration: 130; easing.type: Easing.OutQuad } }

            Repeater {
                model: CharacterData.eyes.length
                delegate: Item {
                    id: eyeChain
                    required property int index

                    // Frame base del ojo (coordenadas viewBox absolutas)
                    readonly property var frame: CharacterData.eyes[index]
                    readonly property real sign: index === 0 ? -1.0 : 1.0
                    // Canales de pose -> centro/escala/rotacion de ESTE ojo
                    readonly property real pcx: (frame.cx + chara.pose.edx * sign) * chara.u
                    readonly property real pcy: (frame.cy + chara.pose.edy
                        + (index === 1 ? chara.pose.edy2 : 0)) * chara.u
                    readonly property real sx: chara.pose.esx + (index === 1 ? chara.pose.esx2 : 0)
                    readonly property real sy: (chara.pose.esy + (index === 1 ? chara.pose.esy2 : 0))
                        * chara.blinkScale
                    readonly property real rotPose: frame.rot * (1 - chara.pose.lock)
                        + (chara.pose.tilt + (index === 1 ? chara.pose.tilt2 : 0)) * sign

                    // A: trasladar al centro poseado del ojo
                    x: pcx
                    y: pcy
                    width: 0
                    height: 0

                    // B: rotacion combinada (tilt de pose + resto de rot base)
                    Item {
                        rotation: eyeChain.rotPose
                        // C: escala (blink incluido) sobre el centro del ojo
                        Item {
                            transform: Scale { xScale: eyeChain.sx; yScale: Math.max(0.02, eyeChain.sy) }
                            // D: anula la rotacion horneada en el path
                            Item {
                                rotation: -eyeChain.frame.rot
                                Shape {
                                    // El path esta en coords absolutas del
                                    // viewBox (0-100), no en px: desplazar el
                                    // Shape para centrar el ojo en 0,0 y
                                    // reescalar viewBox -> px con u.
                                    x: -eyeChain.frame.cx * chara.u
                                    y: -eyeChain.frame.cy * chara.u
                                    width: chara.width
                                    height: chara.height
                                    preferredRendererType: Shape.CurveRenderer
                                    transform: Scale {
                                        origin.x: 0
                                        origin.y: 0
                                        xScale: chara.u
                                        yScale: chara.u
                                    }
                                    ShapePath {
                                        fillColor: eyeChain.frame.fill
                                        strokeColor: "transparent"
                                        PathSvg { path: eyeChain.frame.d }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // === Rock: balanceo continuo mientras pose.rock > 0 (thinking) ===
    QtObject {
        id: rockAnimator
        property real angle: 0
    }
    SequentialAnimation {
        running: chara.pose.rock > 0.01
        loops: Animation.Infinite
        onRunningChanged: if (!running) rockAnimator.angle = 0
        NumberAnimation {
            target: rockAnimator; property: "angle"
            from: -3 * chara.pose.rock; to: 3 * chara.pose.rock
            duration: 850; easing.type: Easing.InOutSine
        }
        NumberAnimation {
            target: rockAnimator; property: "angle"
            from: 3 * chara.pose.rock; to: -3 * chara.pose.rock
            duration: 850; easing.type: Easing.InOutSine
        }
    }

    // === Shake: jitter horizontal cuando pose.shake > 0 ===
    QtObject {
        id: shakeAnimator
        property real offset: 0
    }
    Timer {
        interval: 85
        repeat: true
        running: chara.pose.shake > 0.01
        onRunningChanged: if (!running) shakeAnimator.offset = 0
        onTriggered: {
            var amp = chara.pose.shake * 1.1 * chara.u
            shakeAnimator.offset = (Math.random() * 2 - 1) * amp
        }
    }

    // === Zzz: visible cuando sleeping (lo conduce IdleBehavior) ===
    Item {
        id: zzzLayer
        visible: chara.sleeping
        x: (CharacterData.body.right - 6) * chara.u
        y: (CharacterData.body.top - 16) * chara.u
        width: chara.width
        height: chara.height / 2

        Repeater {
            model: 3
            delegate: Text {
                required property int index
                text: "z"
                font.pixelSize: (11 + index * 5) * chara.u / 1.4
                font.weight: Font.DemiBold
                color: "#9fc6e8"
                opacity: 0

                SequentialAnimation on opacity {
                    running: chara.sleeping
                    loops: Animation.Infinite
                    PauseAnimation { duration: index * 750 }
                    NumberAnimation { from: 0; to: 0.95; duration: 420 }
                    NumberAnimation { from: 0.95; to: 0; duration: 1300 }
                    PauseAnimation { duration: (2 - index) * 750 + 500 }
                }
                SequentialAnimation on y {
                    running: chara.sleeping
                    loops: Animation.Infinite
                    PauseAnimation { duration: index * 750 }
                    NumberAnimation { from: index * -6; to: -34 - index * 10; duration: 1720; easing.type: Easing.OutQuad }
                    PauseAnimation { duration: (2 - index) * 750 + 500 }
                }
            }
        }
    }
}
