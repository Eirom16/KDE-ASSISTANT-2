// IntentRouter.qml - Router determinista: tool call → visual intent → escena.
// 
// Fase 6: whitelist de tools permitidas, fallback seguro, prioridades,
// cancelación end-to-end. Separa lógica del agente (Rust) de animación (QML).
//
// Contrato visual (visual intent):
//   { tool: string, visual: string, target?: string, mood?: string,
//     priority?: string, cancelable?: bool, params?: object }

import QtQuick

QtObject {
    id: router

    // === Referencias ===
    property var characterCtrl: null
    property var sceneCtrl: null

    // === Whitelist de tools que generan intención visual ===
    // Tools no listadas → fallback "system"
    readonly property var allowedTools: ({
        "open_app":      { visual: "openapp",    priority: "IMPORTANT", cancelable: true },
        "create_file":   { visual: "file",       priority: "NORMAL",    cancelable: true },
        "edit_file":     { visual: "file",       priority: "NORMAL",    cancelable: true },
        "read_file":     { visual: "file",       priority: "NORMAL",    cancelable: true },
        "find_file":     { visual: "search",     priority: "NORMAL",    cancelable: true },
        "web_search":    { visual: "search",     priority: "NORMAL",    cancelable: true },
        "show_image":    { visual: "success",    priority: "NORMAL",    cancelable: false },
        "open_file":     { visual: "file",       priority: "NORMAL",    cancelable: true },
        "copy_file":     { visual: "file",       priority: "NORMAL",    cancelable: true },
        "find_document": { visual: "search",     priority: "NORMAL",    cancelable: true },
        "preview_document": { visual: "file",    priority: "NORMAL",    cancelable: true },
        "open_url":      { visual: "openapp",    priority: "NORMAL",    cancelable: true },
        "list_open_apps": { visual: "system",    priority: "NORMAL",    cancelable: false },
        "focus_app":     { visual: "openapp",    priority: "NORMAL",    cancelable: true },
        "close_app":     { visual: "openapp",    priority: "NORMAL",    cancelable: true },
        "system_info":   { visual: "system",     priority: "NORMAL",    cancelable: false },
        "notify":        { visual: "success",    priority: "NORMAL",    cancelable: false },
        "media":         { visual: "system",     priority: "NORMAL",    cancelable: true },
        "volume":        { visual: "system",     priority: "NORMAL",    cancelable: true },
        "brightness":    { visual: "system",     priority: "NORMAL",    cancelable: true },
        "network_status":{ visual: "system",     priority: "NORMAL",    cancelable: false },
        "remind_in":     { visual: "success",    priority: "NORMAL",    cancelable: true },
        "kdeconnect":    { visual: "system",     priority: "NORMAL",    cancelable: true }
    })

    // === Estado ===
    property string _currentIntentTool: ""
    property int _currentPriority: 0

    readonly property var priorityLevels: ({
        "IDLE": 0, "NORMAL": 1, "IMPORTANT": 2, "SECURITY": 3, "ERROR": 4, "CRITICAL": 5
    })

    // === Signals ===
    signal intentReceived(string tool, var intent)
    signal intentStarted(string tool, string visual)
    signal intentFinished(string tool)
    signal intentCancelled(string tool)
    signal intentError(string tool, string error)

    // === API principal: procesar tool call del backend ===
    function processToolCall(toolName, toolCallId, args, approvalNeeded) {
        // Validar tool en whitelist
        var toolDef = allowedTools[toolName]
        if (!toolDef) {
            // Fallback seguro
            toolDef = { visual: "system", priority: "NORMAL", cancelable: true }
        }

        var prio = priorityLevels[toolDef.priority] || 1

        // Verificar prioridad (no interrumpir escena de mayor prioridad)
        if (prio < _currentPriority) {
            intentError(toolName, "prioridad inferior a escena actual")
            return false
        }

        // Construir intención visual
        var intent = {
            tool: toolName,
            toolCallId: toolCallId,
            visual: toolDef.visual,
            priority: toolDef.priority,
            cancelable: toolDef.cancelable,
            args: args || {},
            approvalNeeded: approvalNeeded === true,
            timestamp: Date.now()
        }

        _currentIntentTool = toolName
        _currentPriority = prio

        intentReceived(toolName, intent)

        // Si necesita aprobación, pausar hasta respuesta
        if (intent.approvalNeeded) {
            _waitForApproval(intent)
            return true
        }

        _executeIntent(intent)
        return true
    }

    function _waitForApproval(intent) {
        // La UI muestra badge de confirmación (ChatView toolCalls status="confirm")
        // Cuando el usuario aprueba/deniega, el backend reanuda y llama de nuevo
        // con approvalNeeded=false o envía tool_result directo.
        // Aquí solo registramos la intención pendiente.
        console.log("IntentRouter: esperando aprobación para", intent.tool)
    }

    function _executeIntent(intent) {
        if (!sceneCtrl) {
            intentError(intent.tool, "SceneController no disponible")
            return
        }

        var visual = intent.visual
        var params = intent.args
        params.toolCallId = intent.toolCallId

        var handled = sceneCtrl.play(visual, params)
        if (handled) {
            intentStarted(intent.tool, visual)
        } else {
            intentError(intent.tool, "escena rechazada por prioridad")
        }
    }

    // Llamado cuando el tool termina (éxito o error) desde backend
    function onToolResult(toolCallId, success, result, imageUrl) {
        if (!characterCtrl) return

        // Buscar si coincide con intención actual
        if (toolCallId && _currentIntentTool) {
            // Completar intención
            _finishCurrentIntent(success, result)
        }
    }

    function _finishCurrentIntent(success, result) {
        var tool = _currentIntentTool
        if (!tool) return

        // Disparar escena de resultado según éxito/fallo
        var resultScene = success ? "success" : "error"
        if (sceneCtrl && resultScene) {
            sceneCtrl.play(resultScene, { toolResult: result })
        }

        intentFinished(tool)
        _currentIntentTool = ""
        _currentPriority = 0

        // Liberar busy en CharacterController
        if (characterCtrl) characterCtrl.release("NORMAL")
    }

    // Cancelar intención actual (usuario cancela, barge-in, nueva prioridad)
    function cancel(reason) {
        if (_currentIntentTool) {
            var tool = _currentIntentTool
            if (sceneCtrl) sceneCtrl.cancel()
            intentCancelled(tool)
            _currentIntentTool = ""
            _currentPriority = 0
            if (characterCtrl) characterCtrl.release("NORMAL")
        }
    }

    // Reset total
    function reset() {
        cancel("reset")
        if (sceneCtrl) sceneCtrl.cancel()
    }

    // Obtener definición visual para un tool (para UI/debug)
    function getVisualDef(toolName) {
        return allowedTools[toolName] || { visual: "system", priority: "NORMAL", cancelable: true }
    }
}
