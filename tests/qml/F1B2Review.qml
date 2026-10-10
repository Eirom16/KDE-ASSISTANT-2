import QtQuick
import "F1B2_QML_DIR" as App

App.Main {
    id: chat
    backendUrl: "http://127.0.0.1:8765"
    property string reviewMode: "F1B2_MODE"
    property int phase: 0
    property int waitTicks: 0
    function fail(message) {
        console.error("F1B2_UI_FAIL: " + message)
        Qt.exit(1)
    }
    function checkBackend() {}
    Timer {
        interval: 100
        repeat: true
        running: true
        property int ticks: 0
        onTriggered: {
            ticks++
            if (ticks > 180) { chat.fail("timeout phase " + chat.phase); return }
            if (chat.phase === 0 && ticks >= 5) {
                chat.currentSessionId = "fixture-session"
                chat.messages = [
                    {role:"user", content:"original", raw:"original", toolCalls:[], isStreaming:false},
                    {role:"assistant", content:"completed answer", raw:"completed answer", toolCalls:[], isStreaming:false}
                ]
                if (chat.reviewMode === "stale") {
                    chat.loadSessions()
                    chat.detachChat()
                    chat.currentSessionId = "fixture-new-session"
                    chat.messages = [{role:"assistant", content:"new session marker", raw:"new session marker", isStreaming:false, toolCalls:[]}]
                    chat.waitTicks = ticks
                    chat.phase = 4
                } else if (chat.reviewMode === "regenerate") {
                    chat.regenerate()
                    if (chat.messages.length !== 2 || chat.messages[1].raw !== "completed answer") {
                        chat.fail("regenerate erased completed history before acceptance"); return
                    }
                    if (!chat.showErrorBanner || chat.lastErrorDetail.indexOf("nuevo") < 0) {
                        chat.fail("regenerate restriction unexplained"); return
                    }
                    console.log("F1B2_UI_PASS: regenerate preserves history and explains restriction")
                    Qt.quit()
                } else {
                    chat.streamAgent(chat.backendUrl + "/api/chat", {session_id:chat.currentSessionId, message:"fixture"}, null)
                    chat.phase = 1
                }
            } else if (chat.phase === 1 && chat.activeChatTurnId === "fixture-turn") {
                chat.cancelChat()
                if (!chat.streaming || !chat.chatCancelling) {
                    chat.fail("Stop claims completion before cancellation acknowledgment"); return
                }
                chat.waitTicks = ticks
                chat.phase = 2
            } else if (chat.phase === 2 && ticks - chat.waitTicks >= 10) {
                if (!chat.streaming || !chat.chatCancelling || chat.messages[2].content.indexOf("Cancelando") < 0) {
                    chat.fail("202 is represented as effective cancellation"); return
                }
                var release = new XMLHttpRequest()
                release.open("GET", chat.backendUrl + "/release")
                release.send()
                chat.phase = 3
            } else if (chat.phase === 4 && ticks - chat.waitTicks >= 15) {
                if (chat.currentSessionId !== "fixture-new-session" || chat.messages.length !== 1 || chat.messages[0].raw !== "new session marker") {
                    chat.fail("late session-list callback changed selected session or messages"); return
                }
                console.log("F1B2_UI_PASS: obsolete session-list callback ignored")
                Qt.quit()
            } else if (chat.phase === 3 && !chat.streaming) {
                if (chat.chatCancelling || chat.messages[2].raw.indexOf("Cancelado") < 0) {
                    chat.fail("terminal cancellation missing"); return
                }
                console.log("F1B2_UI_PASS: 202 remains Cancelling until terminal SSE; identity retained")
                Qt.quit()
            }
        }
    }
}
