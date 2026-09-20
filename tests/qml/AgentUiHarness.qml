import QtQuick
import QtQuick.Controls
import qml 1.0
import qml.components 1.0
import qml.agent 1.0

ApplicationWindow {
    id: win
    width: 800
    height: 640
    visible: true

    property bool ok: false
    property string failure: ""

    function assertTrue(condition, message) {
        if (!condition) {
            throw new Error(message)
        }
    }

    function runChecks() {
        picker.openPicker()
        assertTrue(picker.popupVisible(), "model picker did not open")
        picker.selectModel("groq/llama-3.3-70b")
        assertTrue(picker.editText === "groq/llama-3.3-70b", "model picker did not select")
        assertTrue(!picker.popupVisible(), "model picker did not close")

        agent.handleActivity({ type: "intent", phase: "start", run_id: "run-a", tool_call_id: "same-id", tool: "find_file" })
        assertTrue(agent.currentScene === "search", "agent did not start search scene")
        agent.handleActivity({ type: "intent", phase: "start", run_id: "run-b", tool_call_id: "same-id", tool: "open_app" })
        assertTrue(agent.pendingActions.length === 1, "agent did not queue second run")
        agent.handleActivity({ type: "intent", phase: "done", run_id: "run-b", tool_call_id: "same-id", tool: "open_app" })
        assertTrue(agent.currentRunId === "run-a", "agent mixed runs with same tool id")
        agent.resetActivity()

        agent.handleActivity({ type: "intent", phase: "denied", run_id: "run-denied", tool_call_id: "call-denied", tool: "edit_file" })
        assertTrue(agent.activityVisible, "agent did not show denied event")

        settings.open_ = true
        assertTrue(settings.activeTab === "general", "settings initial tab changed")
        settings.activeTab = "voz"
        assertTrue(settings.activeTab === "voz", "settings sidebar tab did not change")
        settings.open_ = false

        dash.stats = {
            active: [{ status: "running", channel: "voice", model: "test", duration_ms: 42, tools: 1, started_at: "2026-01-01T00:00:00Z" }],
            recent: [],
            tool_usage: [{ tool: "find_file", total: 2, success: 1, denied: 1, avg_ms: 15 }],
            completed: 1,
            errors: 0,
            cancelled: 0
        }
        dash.open()
        dash.view = "tools"
        assertTrue(dash.view === "tools", "dashboard view did not switch")
        dash.close()
        ok = true
        console.log("AgentUiHarness: ok")
        Qt.quit()
    }

    ModelPicker {
        id: picker
        objectName: "picker"
        models: ["openai/gpt-4o", "groq/llama-3.3-70b", "anthropic/claude-3.5"]
        anchors.left: parent.left
        anchors.right: parent.right
    }

    AgentWindow {
        id: agent
        objectName: "agent"
        anchors.centerIn: parent
        agentEnabled: true
        reducedMotion: true
        characterSize: 150
    }

    AgentDashboard {
        id: dash
        objectName: "dash"
        backendUrl: "http://127.0.0.1:9"
        autoRefresh: false
    }

    SettingsDialog {
        id: settings
        objectName: "settings"
        anchors.fill: parent
        backendUrl: "http://127.0.0.1:9"
    }

    Timer {
        interval: 250
        running: true
        repeat: false
        onTriggered: {
            try {
                win.runChecks()
            } catch (e) {
                win.failure = String(e)
                console.error("AgentUiHarness:", win.failure)
                Qt.exit(1)
            }
        }
    }
}
