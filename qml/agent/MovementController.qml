// MovementController.qml - Física y animaciones de movimiento del personaje.
// 
// Fase 3: walk/run/jump/bounce/drag/return/appear/disappear con
// aceleración/desaceleración natural, easing, rebote, anticipación,
// cancelación y prioridades.
// Nota: usa implementación JS de timer porque QtQuick.Timers no disponible en qml6.

import QtQuick

QtObject {
    id: ctrl

    // === Configuración de física ===
    property real mass: 1.0
    property real friction: 0.92          // fricción suelo (0-1)
    property real airFriction: 0.98       // fricción aire
    property real gravity: 1200           // px/s²
    property real maxSpeed: 400           // px/s velocidad máxima
    property real walkSpeed: 180          // px/s caminar
    property real runSpeed: 500           // px/s correr
    property real jumpVelocity: -650      // px/s impulso salto (negativo = arriba)
    property real bounceDamping: 0.65     // amortiguación rebote (0-1)
    property real groundY: 0              // posición Y del suelo (px desde arriba ventana)

    // === Estado físico ===
    property real x: 0
    property real y: 0
    property real vx: 0
    property real vy: 0
    property real ax: 0
    property real ay: 0
    property bool onGround: true
    property bool isMoving: false
    property bool isDragging: false

    // Posición base (home) para return
    property real homeX: 0
    property real homeY: 0

    // === Referencia al personaje (para squash/stretch visual) ===
    property var character: null

    // === Animación activa actual ===
    property string _currentAnimation: ""
    property int _animationPriority: 0  // 0=none, 1=idle, 2=walk, 3=run, 4=jump, 5=scene, 6=drag

    readonly property var animPriority: ({
        "none": 0, "idle": 1, "walk": 2, "run": 3, "jump": 4, "bounce": 4,
        "scene": 5, "drag": 6, "return": 2, "appear": 5, "disappear": 5
    })

    // === Signals ===
    signal positionChanged(real x, real y)
    signal animationStarted(string name, int priority)
    signal animationFinished(string name)
    signal reachedDestination()
    signal fellToGround()

    // === Timer de simulación física (60 FPS) ===
    // El Timer vive FUERA de este componente (en AgentWindow, que es un Item):
    // un QtObject no puede tener Items hijos. Aquí solo la API.
    property var _physicsTimer: null    // lo asigna quien lo crea (AgentWindow)
    property bool _physicsRunning: false
    property var _lastTime: 0

    function _startPhysicsTimer() {
        _lastTime = Date.now()
        _physicsRunning = true
        if (_physicsTimer) _physicsTimer.running = true
    }

    function _stopPhysicsTimer() {
        _physicsRunning = false
        if (_physicsTimer) _physicsTimer.running = false
    }

    function _physicsTick() {
        var now = Date.now()
        var dt = (now - _lastTime) / 1000.0
        _lastTime = now
        _physicsStep(dt)
    }

    Component.onCompleted: {
        _physicsRunning = true
        _lastTime = Date.now()
        if (_physicsTimer) _physicsTimer.running = true
    }

    Component.onDestruction: {
        _stopPhysicsTimer()
    }

    function _physicsStep(dt) {
        if (isDragging) return  // arrastre manual pausa física

        // Gravedad si no en suelo
        if (!onGround) {
            ay = gravity
        } else {
            ay = 0
        }

        // Integración Euler simple
        vx += ax * dt
        vy += ay * dt

        // Fricción
        if (onGround) {
            vx *= friction
        } else {
            vx *= airFriction
            vy *= airFriction
        }

        // Límite velocidad
        var speed = Math.sqrt(vx * vx + vy * vy)
        if (speed > maxSpeed) {
            vx = vx * maxSpeed / speed
            vy = vy * maxSpeed / speed
        }

        // Posición
        x += vx * dt
        y += vy * dt

        // Suelo
        if (y >= groundY) {
            y = groundY
            if (!onGround && vy > 50) {
                // Aterrizaje con rebote
                vy = -vy * bounceDamping
                onGround = false
                // Trigger squash visual
                if (character) {
                    character.squashX = 1.15
                    character.squashY = 0.85
                    // Recuperar forma
                    Qt.callLater(function() {
                        if (character) {
                            character.squashX = 1.0
                            character.squashY = 1.0
                        }
                    })
                }
            } else {
                onGround = true
                vy = 0
            }
        }

        // Frenar si velocidad muy baja
        if (Math.abs(vx) < 10) vx = 0

        positionChanged(x, y)
        _updateVisuals(dt)
    }

    function _updateVisuals(dt) {
        if (!character) return

        // Squash/stretch por velocidad horizontal
        var speedRatio = Math.abs(vx) / maxSpeed
        if (onGround && isMoving) {
            character.squashX = 1.0 - speedRatio * 0.12
            character.squashY = 1.0 + speedRatio * 0.18
        } else if (!onGround) {
            // Estiramiento en aire
            character.squashX = 1.0 + Math.abs(vy) / 800 * 0.15
            character.squashY = 1.0 - Math.abs(vy) / 800 * 0.10
        } else {
            // Recuperación suave a 1.0
            character.squashX += (1.0 - character.squashX) * 0.15
            character.squashY += (1.0 - character.squashY) * 0.15
        }

        // Postura por dirección de movimiento
        if (Math.abs(vx) > 20) {
            character.postureRot = vx > 0 ? -5 : 5
        } else {
            // Dejar que IdleBehavior maneje postureRot en idle
        }
    }

    // === API pública de movimiento ===

    // Mover a posición con walk/run (prioridad 2/3)
    function moveTo(targetX, targetY, mode, priorityName) {
        var prio = animPriority[mode] !== undefined ? animPriority[mode] : 2
        if (priorityName && animPriority[priorityName] !== undefined) {
            prio = animPriority[priorityName]
        }
        if (prio < _animationPriority && _animationPriority !== animPriority.drag) {
            return false
        }
        _animationPriority = prio
        _currentAnimation = mode

        isMoving = true
        var dx = targetX - x
        var dy = targetY - y
        var dist = Math.sqrt(dx * dx + dy * dy)

        if (dist < 5) {
            _finishAnimation(mode)
            return true
        }

        var targetSpeed = (mode === "run") ? runSpeed : walkSpeed
        var dirX = dx / dist
        var dirY = dy / dist

        // Aceleración suave (anticipación)
        ax = dirX * targetSpeed * 3.0
        ay = dirY * targetSpeed * 3.0

        animationStarted(mode, prio)
        return true
    }

    // Caminar hacia
    function walkTo(targetX, targetY) { return moveTo(targetX, targetY, "walk") }

    // Correr hacia
    function runTo(targetX, targetY) { return moveTo(targetX, targetY, "run") }

    // Saltar (solo si en suelo)
    function jump() {
        if (!onGround) return false
        if (animPriority.jump < _animationPriority) return false

        _animationPriority = animPriority.jump
        _currentAnimation = "jump"
        vy = jumpVelocity
        onGround = false
        isMoving = true

        // Anticipación visual: squash antes del salto
        if (character) {
            character.squashX = 1.2
            character.squashY = 0.75
        }

        animationStarted("jump", animPriority.jump)
        return true
    }

    // Rebote (bounce en lugar)
    function bounce(intensity) {
        if (animPriority.bounce < _animationPriority) return false
        intensity = intensity !== undefined ? intensity : 1.0

        _animationPriority = animPriority.bounce
        _currentAnimation = "bounce"

        vy = -300 * intensity
        onGround = false
        isMoving = true

        // Squash visual
        if (character) {
            character.squashX = 1.1 + intensity * 0.15
            character.squashY = 0.85 - intensity * 0.1
        }

        animationStarted("bounce", animPriority.bounce)
        return true
    }

    // Regresar a home
    function returnHome() {
        if (animPriority.return < _animationPriority && _animationPriority !== animPriority.drag) return false
        _animationPriority = animPriority.return
        _currentAnimation = "return"
        isMoving = true
        moveTo(homeX, homeY, "walk", "return")
        return true
    }

    // Establecer home
    function setHome(x, y) {
        homeX = x
        homeY = y
    }

    // Aparecer (spawn con animación)
    function appear(x, y, fromDirection) {
        _animationPriority = animPriority.appear
        _currentAnimation = "appear"
        x = x
        y = y
        vx = 0
        vy = 0
        onGround = true

        // Animación de entrada: scale 0->1 + slide
        if (character) {
            character.scale = 0.0
            character.opacity = 0.0

            var anim = Qt.createQmlObject('import QtQuick; SequentialAnimation { PropertyAnimation { target: ctrl.character; property: "scale"; from: 0; to: 1; duration: 400; easing.type: Easing.OutBack } PropertyAnimation { target: ctrl.character; property: "opacity"; from: 0; to: 1; duration: 300; easing.type: Easing.OutQuad } }', ctrl, "appearAnim")
            if (anim) anim.start()
        }

        animationStarted("appear", animPriority.appear)
        return true
    }

    // Desaparecer
    function disappear() {
        _animationPriority = animPriority.disappear
        _currentAnimation = "disappear"

        if (character) {
            var anim = Qt.createQmlObject('import QtQuick; SequentialAnimation { PropertyAnimation { target: ctrl.character; property: "scale"; from: 1; to: 0; duration: 300; easing.type: Easing.InBack } PropertyAnimation { target: ctrl.character; property: "opacity"; from: 1; to: 0; duration: 200; easing.type: Easing.InQuad } ScriptAction { script: ctrl.animationFinished("disappear") } }', ctrl, "disappearAnim")
            if (anim) anim.start()
        } else {
            animationFinished("disappear")
        }
        return true
    }

    // Iniciar arrastre (usuario)
    function startDrag(x, y) {
        isDragging = true
        isMoving = false
        vx = 0
        vy = 0
        onGround = true
        _animationPriority = animPriority.drag
        _currentAnimation = "drag"

        // Posición inicial del drag
        x = x
        y = y

        if (character) {
            character.squashX = 1.1
            character.squashY = 0.9
        }
        return true
    }

    // Actualizar arrastre
    function updateDrag(x, y) {
        if (!isDragging) return
        x = x
        y = y
        positionChanged(x, y)
    }

    // Terminar arrastre (soltar -> física)
    function endDrag(throwVx, throwVy) {
        if (!isDragging) return
        isDragging = false
        vx = throwVx !== undefined ? throwVx : 0
        vy = throwVy !== undefined ? throwVy : 0
        onGround = false
        _animationPriority = 0
        _currentAnimation = ""
        return true
    }

    // Cancelar animación actual
    function cancel() {
        ax = 0
        ay = 0
        vx = 0
        vy = 0
        isMoving = false
        _animationPriority = 0
        _currentAnimation = ""
        if (character) {
            character.squashX = 1.0
            character.squashY = 1.0
        }
    }

    // Pausar/reanudar (para escenas)
    function pause() { _stopPhysicsTimer() }
    function resume() { _startPhysicsTimer() }

    // Verificar si se llegó al destino (para walk/run)
    function _checkArrival(targetX, targetY) {
        var dx = targetX - x
        var dy = targetY - y
        return Math.sqrt(dx * dx + dy * dy) < 15 && Math.abs(vx) < 20
    }

    function _finishAnimation(name) {
        isMoving = false
        _animationPriority = 0
        _currentAnimation = ""
        if (character) {
            character.squashX = 1.0
            character.squashY = 1.0
        }
        animationFinished(name)
    }
}
