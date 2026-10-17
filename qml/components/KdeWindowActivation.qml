import QtQuick
import org.kde.kwindowsystem 1.0

QtObject {
    readonly property bool wayland: KWindowSystem.isPlatformWayland
    function activate(window, token) {
        if (token && KWindowSystem.isPlatformWayland) {
            if (typeof KWindowSystem.setCurrentXdgActivationToken !== "function") return false
            KWindowSystem.setCurrentXdgActivationToken(token)
        }
        // Public KDE API; KWin still decides whether this is focus stealing.
        KWindowSystem.activateWindow(window)
        return true
    }
}
