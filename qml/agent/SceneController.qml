// SceneController.qml - Orquesta escenas animadas para representar
// acciones del asistente (búsqueda, apertura apps, archivos, etc.).
//
// Fase 5: escenas coreografiadas = secuencia de estados visuales
// (expresión, mood, movimiento, mirada) con duración y cancelación.

import QtQuick

QtObject {
    id: ctrl

    // === Referencias a subsistemas ===
    property var characterCtrl: null
    property var expressionCtrl: null
    property var animationCtrl: null
    property var moodCtrl: null
    property var gazeCtrl: null
    property var movementCtrl: null

    // === Estado de escena actual ===
    property string _currentScene: ""
    property var _currentSceneObj: null
    property int _scenePriority: 0

    // Prioridades de escena
    readonly property var scenePriority: ({
        "none": 0, "idle": 1, "search": 2, "openapp": 3, "file": 3,
        "system": 3, "error": 4, "success": 3, "download": 3, "screenshot": 3
    })

    // === Signals ===
    signal sceneStarted(string name, int priority)
    signal sceneFinished(string name)
    signal sceneCancelled(string name)

    // === Registro de escenas (instanciadas bajo demanda) ===
    property var _sceneCache: ({})

    // === API pública ===

    // Ejecutar una escena por nombre
    function play(name, params) {
        var prio = scenePriority[name] !== undefined ? scenePriority[name] : 2
        if (prio < _scenePriority) {
            console.log("SceneController:", name, "rechazada (prioridad", prio, "<", _scenePriority, ")")
            return false
        }

        // Cancelar escena actual si existe
        if (_currentScene) {
            _cancelCurrentScene()
        }

        var sceneObj = _getScene(name)
        if (!sceneObj) {
            console.warn("SceneController: escena desconocida:", name)
            return false
        }

        _currentScene = name
        _currentSceneObj = sceneObj
        _scenePriority = prio

        // Pausar idle autónomo durante la escena
        if (characterCtrl) characterCtrl.busy = true

        sceneObj.start(params)
        sceneStarted(name, prio)
        return true
    }

    function _getScene(name) {
        if (_sceneCache[name]) return _sceneCache[name]

        var sceneObj = null
        switch (name) {
        case "search": sceneObj = Qt.createQmlObject('import QtQuick; SearchScene { }', ctrl, "searchScene"); break
        case "openapp": sceneObj = Qt.createQmlObject('import QtQuick; OpenAppScene { }', ctrl, "openAppScene"); break
        case "file": sceneObj = Qt.createQmlObject('import QtQuick; FileScene { }', ctrl, "fileScene"); break
        case "system": sceneObj = Qt.createQmlObject('import QtQuick; SystemScene { }', ctrl, "systemScene"); break
        case "error": sceneObj = Qt.createQmlObject('import QtQuick; ErrorScene { }', ctrl, "errorScene"); break
        case "success": sceneObj = Qt.createQmlObject('import QtQuick; SuccessScene { }', ctrl, "successScene"); break
        case "download": sceneObj = Qt.createQmlObject('import QtQuick; DownloadScene { }', ctrl, "downloadScene"); break
        case "screenshot": sceneObj = Qt.createQmlObject('import QtQuick; ScreenshotScene { }', ctrl, "screenshotScene"); break
        }

        if (sceneObj) {
            _injectDependencies(sceneObj)
            _sceneCache[name] = sceneObj
        }
        return sceneObj
    }

    function _injectDependencies(obj) {
        obj.characterCtrl = ctrl.characterCtrl
        obj.expressionCtrl = ctrl.expressionCtrl
        obj.animationCtrl = ctrl.animationCtrl
        obj.moodCtrl = ctrl.moodCtrl
        obj.gazeCtrl = ctrl.gazeCtrl
        obj.movementCtrl = ctrl.movementCtrl
    }

    function _cancelCurrentScene() {
        if (_currentSceneObj && _currentSceneObj.cancel) {
            _currentSceneObj.cancel()
        }
        if (characterCtrl) characterCtrl.busy = false
        sceneCancelled(_currentScene)
        _currentScene = ""
        _currentSceneObj = null
        _scenePriority = 0
    }

    // Cancelar escena actual
    function cancel() {
        if (_currentScene) {
            _cancelCurrentScene()
        }
    }

    // Completar escena actual (llamado por la escena al terminar)
    function complete(name) {
        if (_currentScene === name) {
            if (characterCtrl) characterCtrl.busy = false
            sceneFinished(name)
            _currentScene = ""
            _currentSceneObj = null
            _scenePriority = 0
        }
    }

    // Verificar si hay escena activa
    function isPlaying(name) {
        return _currentScene === name
    }

    // Mapeo tool → escena (usado por CharacterController/IntentRouter en Fase 6)
    readonly property var toolToScene: ({
        "open_app": "openapp",
        "create_file": "file",
        "edit_file": "file",
        "read_file": "file",
        "find_file": "search",
        "web_search": "search",
        "show_image": "success",
        "open_file": "file",
        "open_url": "openapp",
        "system_info": "system",
        "notify": "success",
        "media": "system",
        "volume": "system",
        "brightness": "system",
        "network_status": "system",
        "remind_in": "success",
        "kdeconnect": "system"
    })

    function sceneForTool(toolName) {
        return toolToScene[toolName] || "system"
    }
}
