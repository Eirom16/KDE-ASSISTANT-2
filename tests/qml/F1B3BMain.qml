import QtQuick
import "F1B3B_QML_DIR" as App
App.Main {
    id: chat
    backendUrl: "F1B3B_BACKEND"
    property int phase: 0
    property int waitingTicks: 0
    function checkBackend() {}
    function find(item, name) {
        if (item.objectName === name) return item
        if (item.children) for (var i = 0; i < item.children.length; ++i) { var found = find(item.children[i], name); if (found) return found }
        return null
    }
    function fail(message) { console.error("F1B3B_UI_FAIL: " + message); Qt.exit(1) }
    Timer {
        interval: 100
        repeat: true
        running: true
        property int ticks: 0
        onTriggered: {
            ticks++
            if (ticks > 100) { chat.fail("timeout phase " + phase); return }
            if (phase === 0 && ticks >= 3) { chat.currentSessionId = "unknown"; phase = 1 }
            else if (phase === 1 && chat.recoveryCount > 0) { chat.deleteSession("unknown"); phase = 2 }
            else if (phase === 2) {
                var recovery = chat.find(chat.contentItem, "recoveryHistory")
                if (!recovery || !recovery.confirmingDeletion) return
                if (chat.currentSessionId !== "unknown") { chat.fail("deleted before consent"); return }
                waitingTicks = ticks
                phase = 3
            } else if (phase === 3 && ticks - waitingTicks > 5) {
                var button = chat.find(chat.contentItem, "eraseEvidenceButton")
                if (!button) { chat.fail("confirmation action missing"); return }
                button.clicked()
                phase = 4
            } else if (phase === 4 && chat.currentSessionId === "") {
                chat.currentSessionId = "deleted"
                phase = 5
            } else if (phase === 5 && chat.recoveryStatus === "empty") {
                console.log("F1B3B_UI_PASS: session banner, 409 confirmation, explicit deletion and deleted session")
                Qt.quit()
            }
        }
    }
}
