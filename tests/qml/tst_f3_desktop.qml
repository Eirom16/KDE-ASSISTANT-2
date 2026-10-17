import QtQuick
import QtQuick.Window
import QtTest
import qml 1.0 as App
import qml.components 1.0 as Components

Item {
    width: 600; height: 700
    QtObject {
        id: fake
        property bool visible: true
        property bool active: true
        property int visibility: Window.Windowed
        property string lastShow: ""
        property int activationRequests: 0
        function show() { visible = true; lastShow = "show" }
        function hide() { visible = false }
        function showNormal() { visible = true; visibility = Window.Windowed; lastShow = "normal" }
        function showMaximized() { visible = true; visibility = Window.Maximized; lastShow = "maximized" }
        function showMinimized() { visibility = Window.Minimized; active = false }
        function raise() {}
        function requestActivate() { activationRequests++ }
    }
    Components.DesktopWindowController { id: controller; window: fake; nativeActivationEnabled: false }
    QtObject {
        id: nativeMock
        readonly property bool wayland: true
        function activate(window, token) { window.activationRequests++; return true }
    }
    App.Main { id: main; backendUrl: "http://127.0.0.1:1" }
    TestCase {
        name: "F3Desktop"
        when: windowShown
        function init() {
            fake.visible = true; fake.active = true; fake.visibility = Window.Windowed
            fake.lastShow = ""; fake.activationRequests = 0
            controller.restoreMaximized = false
            controller.nativeActivator = null
            main.detachChat()
        }
        function cleanup() { main.detachChat() }
        function consent() {
            main.pendingConsents = ({"f3-call": {tool_call_id: "f3-call", consent_id: "f3-consent",
                session_id: "f3-session", turn_id: "f3-turn", name: "edit_file",
                permission: "red", effect: "Editar fixture", resource: "/synthetic/f3.txt"}})
            main.showConsent("f3-call")
        }
        function test_active_hides_hidden_restores() {
            controller.toggle(); compare(fake.visible, false)
            fake.active = false
            controller.toggle(); compare(fake.visible, true); compare(fake.lastShow, "normal")
            compare(fake.activationRequests, 1)
        }
        function test_kwin_minimized_qt_windowed_remaps_once() {
            fake.active = false; fake.visibility = Window.Windowed
            controller.toggle()
            compare(fake.lastShow, "normal"); compare(fake.visible, true); compare(fake.activationRequests, 1)
        }
        function test_minimized_restores_once() {
            controller.minimize(); compare(fake.visibility, Window.Minimized)
            controller.toggle(); compare(fake.lastShow, "normal"); compare(fake.activationRequests, 1)
        }
        function test_unfocused_maximized_preserved() {
            fake.visibility = Window.Maximized; controller.rememberState()
            fake.active = false
            controller.present(); compare(fake.lastShow, "maximized")
            controller.hide(); controller.present(); compare(fake.lastShow, "maximized")
            controller.minimize(); controller.present(); compare(fake.lastShow, "maximized")
        }
        function test_native_maximized_remap_preserves_state() {
            controller.nativeActivator = nativeMock
            fake.visibility = Window.Maximized; fake.active = false
            controller.present()
            compare(fake.visible, false)
            tryCompare(fake, "lastShow", "maximized")
            compare(fake.visible, true); compare(fake.activationRequests, 1)
        }
        function test_hide_cancels_deferred_native_presentation() {
            controller.nativeActivator = nativeMock
            fake.active = false
            controller.present(); controller.hide()
            wait(30)
            compare(fake.visible, false); compare(fake.activationRequests, 0)
        }
        function test_user_returns_to_normal() {
            fake.visibility = Window.Maximized; controller.rememberState()
            fake.visibility = Window.Windowed; fake.active = true; controller.rememberState()
            fake.active = false; controller.present(); compare(fake.lastShow, "normal")
        }
        function test_new_session_revokes_pending_ui_consent() {
            consent()
            var dialog = findChild(main, "f2ConsentDialog")
            tryCompare(dialog, "visible", true)
            main.newSession()
            compare(Object.keys(main.pendingConsents).length, 0)
            tryCompare(dialog, "visible", false)
        }
        function test_hidden_consent_becomes_visible_never_autoapproves() {
            main.hide(); consent()
            tryCompare(main, "visible", true)
            var dialog = findChild(main, "f2ConsentDialog")
            tryCompare(dialog, "visible", true)
            compare(Object.keys(main.pendingConsents).length, 1)
            main.handleDesktopAction("execute_tool")
            compare(Object.keys(main.pendingConsents).length, 1)
            main.detachChat()
            compare(Object.keys(main.pendingConsents).length, 0)
        }
        function test_close_hides_own_window() {
            main.show(); main.closeToTray = true
            tryCompare(main, "visible", true)
            wait(50)
            main.close()
            tryCompare(main, "visible", false)
            wait(1600)
            compare(main.visible, false)
            main.handleDesktopAction("show"); tryCompare(main, "visible", true)
        }
    }
}
