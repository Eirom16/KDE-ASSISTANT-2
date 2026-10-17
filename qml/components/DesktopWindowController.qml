import QtQuick
import QtQuick.Window

QtObject {
    id: controller
    required property var window
    property bool restoreMaximized: false
    property bool nativeActivationEnabled: true
    property var nativeActivator: null
    property string nativeActivationStatus: "qt"
    property int presentationGeneration: 0
    property bool userInteracted: false
    Component.onCompleted: {
        if (!nativeActivationEnabled) return
        var component = Qt.createComponent(Qt.resolvedUrl("KdeWindowActivation.qml"))
        if (component.status === Component.Ready) {
            nativeActivator = component.createObject(controller)
            nativeActivationStatus = nativeActivator ? "kde" : "NOT_SUPPORTED"
        } else nativeActivationStatus = "NOT_SUPPORTED"
        if (nativeActivationStatus === "NOT_SUPPORTED")
            console.warn("F3: KWindowSystem no disponible; activación nativa no garantizada")
    }
    readonly property bool canHide: window.visible && window.active && window.visibility !== Window.Minimized

    function rememberState() {
        if (window.visibility === Window.Maximized) restoreMaximized = true
        else if (window.visibility === Window.Windowed && window.active) restoreMaximized = false
    }
    function present(token) {
        userInteracted = true
        // KWin may minimize while Qt still reports Windowed. Remap an inactive
        // window, preserving its last maximized state rather than losing it.
        var maximized = restoreMaximized || window.visibility === Window.Maximized
        console.info("F3_WINDOW_PRESENT", Date.now(), window.visible, window.active, window.visibility, maximized)
        var generation = ++presentationGeneration
        // KWin can keep it minimized while Qt reports Windowed. A new mapping
        // on a later event-loop turn resets that compositor state using Qt's
        // supported lifecycle. Preserve the QML tree and maximized state.
        if (nativeActivator && nativeActivator.wayland && window.visible && !window.active) {
            window.hide()
            Qt.callLater(function() {
                if (generation !== presentationGeneration) return
                finishPresentation(maximized, token)
            })
            return
        }
        finishPresentation(maximized, token)
    }
    function finishPresentation(maximized, token) {
        if (!window.active || window.visibility === Window.Minimized || !window.visible) {
            if (maximized) window.showMaximized()
            else window.showNormal()
        } else window.show()
        window.raise()
        if (!nativeActivator || !nativeActivator.activate(window, token || "")) window.requestActivate()
    }
    function hide() {
        userInteracted = true
        ++presentationGeneration
        console.info("F3_WINDOW_HIDE", Date.now(), window.visible, window.active, window.visibility)
        if (window.visibility === Window.Maximized) restoreMaximized = true
        window.hide()
    }
    function minimize() {
        userInteracted = true
        ++presentationGeneration
        if (window.visibility === Window.Maximized) restoreMaximized = true
        window.showMinimized()
    }
    function toggle(token) { if (canHide) hide(); else present(token) }
}
