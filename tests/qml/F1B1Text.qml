// Harness de flujo real de Main.qml y SettingsDialog, sólo offscreen.
import QtQuick
import "F1B1_QML_DIR" as App
import "F1B1_COMPONENT_DIR" as Components

App.Main {
    id: chat
    backendUrl: "F1B1_BACKEND"
    property int phase: 0
    property bool settingsSaved: false
    Components.SettingsDialog {
        id: settings
        backendUrl: chat.backendUrl
        onSaved: chat.settingsSaved = true
    }
    function fail(message) {
        console.error("F1B1_UI_FAIL: " + message)
        Qt.exit(1)
    }
    Timer {
        interval: 100
        repeat: true
        running: true
        property int ticks: 0
        onTriggered: {
            ticks++
            if (ticks > 150) { chat.fail("timeout phase " + chat.phase); return }
            if (chat.phase === 0 && chat.backendStatus === "nokey") {
                if (!chat.showErrorBanner) { chat.fail("missing key is invisible"); return }
                settings.show()
                chat.phase = 1
            } else if (chat.phase === 1 && settings.rawConfig.ai) {
                if (settings.rawConfig.ai.api_key !== "") { chat.fail("secret returned"); return }
                settings.provider = "custom"
                settings.providerKeys = ({ custom: "fixture-key" })
                settings.providerBaseUrls = ({ custom: "F1B1_PROVIDER" })
                settings.providerModels = ({ custom: "mock-text" })
                settings.applyProviderFields("custom")
                settings.saveConfig()
                chat.phase = 2
            } else if (chat.phase === 2 && chat.settingsSaved) {
                chat.checkBackend()
                chat.sendMessage("first")
                chat.phase = 3
            } else if (chat.phase === 3 && !chat.streaming && chat.messages.length === 2) {
                if (chat.messages[0].content !== "first" || chat.messages[1].content.indexOf("reply-one") < 0) {
                    chat.fail("first response missing from chat"); return
                }
                chat.sendMessage("second")
                chat.phase = 4
            } else if (chat.phase === 4 && !chat.streaming && chat.messages.length === 4) {
                if (chat.messages[2].content !== "second" || chat.messages[3].content.indexOf("reply-two") < 0) {
                    chat.fail("second response missing from chat"); return
                }
                if (!chat.currentSessionId) { chat.fail("no session"); return }
                console.log("F1B1_UI_PASS: settings, session, two turns, four visible messages")
                Qt.quit()
            }
        }
    }
}
