import QtQuick
import QtTest
import qml 1.0 as App

Item {
    App.Main {
        id: main
        backendUrl: "http://127.0.0.1:1"
        visible: true
    }

    TestCase {
        name: "F2Consent"
        when: windowShown
        function test_exact_effect_and_close_denies() {
            main.pendingConsents = ({"local-call": {
                tool_call_id: "local-call", consent_id: "consent-fixture",
                turn_id: "turn-fixture", session_id: null,
                name: "read_file", effect: "Leer archivo y enviar su texto al proveedor de IA",
                resource: "/private/fixture.txt", permission: "yellow"
            }})
            main.showConsent("local-call")
            var dialog = findChild(main, "f2ConsentDialog")
            verify(dialog !== null)
            tryCompare(dialog, "visible", true)
            verify(findChild(dialog, "f2ConsentTool").text.indexOf("read_file") >= 0)
            verify(findChild(dialog, "f2ConsentEffect").text.indexOf("proveedor") >= 0)
            verify(findChild(dialog, "f2ConsentResource").text.indexOf("fixture.txt") >= 0)
            dialog.close()
            tryCompare(dialog, "visible", false)
            compare(Object.keys(main.pendingConsents).length, 0)
        }

        function test_session_detach_discards_old_consent() {
            main.pendingConsents = ({"old-call": {
                tool_call_id: "old-call", consent_id: "old-consent",
                turn_id: "old-turn", session_id: "old-session",
                name: "edit_file", effect: "Sobrescribir archivo",
                resource: "/synthetic/old.txt", permission: "red"
            }})
            main.showConsent("old-call")
            var dialog = findChild(main, "f2ConsentDialog")
            tryCompare(dialog, "visible", true)
            main.detachChat()
            tryCompare(dialog, "visible", false)
            compare(Object.keys(main.pendingConsents).length, 0)
        }
    }
}
