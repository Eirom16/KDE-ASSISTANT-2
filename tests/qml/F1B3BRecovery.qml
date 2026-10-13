import QtQuick
import QtQuick.Window
import qml 1.0 as App
Window {
    id: harness
    width: 760
    height: 680
    visible: true
    property int phase: 0
    property int confirmed: 0
    function fail(message) { console.error("F1B3B_UI_FAIL: " + message); Qt.exit(1) }
    App.RecoveryHistory {
        id: recovery
        anchors.fill: parent
        backendUrl: "F1B3B_BACKEND"
        authToken: "synthetic-fixture"
        onEraseConfirmed: function(session) { if (session !== "unknown") harness.fail("wrong delete identity"); harness.confirmed++ }
    }
    Timer {
        interval: 100
        repeat: true
        running: true
        property int ticks: 0
        onTriggered: {
            ticks++
            if (ticks > 120) { harness.fail("timeout phase " + phase); return }
            if (phase === 0) { recovery.show("unknown"); phase = 1 }
            else if (phase === 1 && recovery.status === "ready") {
                if (recovery.entries[0].state !== "unknown_outcome" || recovery.uncertainExplanation !== "La operación pudo haberse realizado, pero no se pudo confirmar su resultado.") { harness.fail("uncertainty missing"); return }
                if (recovery.explanation("failed").indexOf("parciales") < 0 || recovery.explanation("abandoned").indexOf("antes") < 0) { harness.fail("status semantics missing"); return }
                recovery.show("legacy"); phase = 2
            } else if (phase === 2 && recovery.status === "ready") {
                if (recovery.entries[0].id !== undefined) { harness.fail("legacy fixture unexpectedly complete"); return }
                recovery.show("empty"); phase = 3
            } else if (phase === 3 && recovery.status === "empty") { recovery.show("error"); phase = 4 }
            else if (phase === 4 && recovery.status === "unavailable") {
                recovery.confirmDeletion("unknown")
                if (!recovery.confirmingDeletion || recovery.deletionExplanation.indexOf("investigar") < 0 || confirmed !== 0) { harness.fail("deletion consent missing"); return }
                recovery.hide()
                if (confirmed !== 0) { harness.fail("dismissal deleted evidence"); return }
                recovery.show("slow")
                recovery.show("empty")
                phase = 5
            } else if (phase === 5 && ticks > 30) {
                if (recovery.status !== "empty" || recovery.entries.length !== 0) { harness.fail("stale response replaced session"); return }
                console.log("F1B3B_UI_PASS: uncertainty, failed, abandoned, legacy, empty, unavailable, consent and stale response")
                Qt.quit()
            }
        }
    }
}
