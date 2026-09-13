// AgentMain.qml - Punto de entrada del proceso nativo Wayland (layer-shell).
// Se ejecuta como proceso separado (spawn desde Rust backend).
// Integra AgentOverlay + AgentWindow (personaje) en un solo árbol QML.
//
// Uso: qml6 -I . qml/agent/AgentMain.qml
// O: QT_QPA_PLATFORM=wayland qml6 -I . qml/agent/AgentMain.qml

import QtQuick
import QtQuick.Window
import "AgentOverlay.qml" as Overlay

// El overlay es la ventana raíz en Wayland nativo
Overlay.AgentOverlay {
    id: overlay

    // Config por defecto (sobrescribible vía propiedades o HTTP)
    characterSize: 140
    presenceMode: "companion"
    reducedMotion: false
    sleepAfterSecs: 240
    clickThrough: false
    interactive: true

    // === Personaje como hijo del overlay ===
    // AgentWindow ahora es un Item (no Window) que se posiciona libremente
    AgentWindow {
        id: characterWindow
        // AgentWindow expone propiedades para integrarse en el overlay
        characterSize: overlay.characterSize
        presenceMode: overlay.presenceMode
        reducedMotion: overlay.reducedMotion
        sleepAfterSecs: overlay.sleepAfterSecs
        assistantState: overlay.assistantState
        voiceLevel: overlay.voiceLevel

        // Posición inicial: abajo-derecha
        x: overlay.width - width - 24
        y: overlay.height - height - 20

        // Reenviar señales del personaje al overlay
        onClickedByUser: overlay.userInteracted("click")
    }

    // === Sincronizar estado con Rust backend (HTTP polling o WebSocket) ===
    // Por ahora: propiedades expuestas para seteo externo
    // En Fase 5+: conexión HTTP/SSE real

    Component.onCompleted: {
        console.log("AgentMain: proceso nativo Wayland iniciado")
        console.log("Overlay:", overlay.width, "x", overlay.height)
        console.log("Character at:", characterWindow.x, characterWindow.y)
    }
}
