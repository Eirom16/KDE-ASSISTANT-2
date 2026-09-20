import QtQuick

// Work stays active until its correlated tool result arrives.
QtObject {
    id: ctrl
    property var characterCtrl: null
    property var expressionCtrl: null
    property var animationCtrl: null
    property var moodCtrl: null
    property var gazeCtrl: null
    property var movementCtrl: null
    property string _currentScene: ""
    property int _scenePriority: 0
    property string tool: ""
    property string toolCallId: ""
    property string runId: ""
    property string phase: ""
    property string pendingResult: ""
    property double startedAt: 0
    property int step: 0
    readonly property bool active: _currentScene !== ""
    readonly property var toolToScene: ({
        open_app:"openapp", open_url:"openapp", find_file:"search", web_search:"search",
        create_file:"file", edit_file:"file", read_file:"file", open_file:"file", copy_file:"file",
        find_document:"search", preview_document:"file",
        list_open_apps:"system", focus_app:"openapp", close_app:"openapp",
        show_image:"download", screenshot:"screenshot", notify:"success", remind_in:"success",
        system_info:"system", network_status:"system", volume:"system", brightness:"system",
        media:"system", kdeconnect:"system"
    })
    signal sceneStarted(string name, int priority)
    signal sceneFinished(string name)
    signal sceneCancelled(string name)
    function sceneForTool(name) { return toolToScene[name] || "system" }
    function isPlaying(name) { return _currentScene === name }
    function play(name, params) {
        cancel()
        params = params || {}
        _currentScene = ["search","openapp","file","system","error","success","download","screenshot"].indexOf(name) >= 0 ? name : "system"
        tool = params.tool || ""
        toolCallId = params.toolCallId || ""
        runId = params.runId || ""
        phase = params.approval ? "approval" : "working"
        pendingResult = ""
        startedAt = Date.now()
        step = 0
        _scenePriority = name === "error" ? 4 : 2
        if (moodCtrl) moodCtrl.setMood(phase === "approval" ? "concerned" : "focused", 0.8, 0)
        animateWork()
        clock.restart()
        sceneStarted(_currentScene, _scenePriority)
        return true
    }
    function animateWork() {
        if (phase !== "working") {
            if (expressionCtrl) expressionCtrl.request("unsure")
            return
        }
        var zone = tool === "web_search" || tool === "open_url" ? "left" : "files"
        if (gazeCtrl) gazeCtrl.lookAtZone(zone, 900)
        if (expressionCtrl) expressionCtrl.request("thinking")
        var motion = "inspect"
        switch (_currentScene) {
        case "search": motion = "search"; break
        case "file": motion = tool === "create_file" || tool === "edit_file" ? "write" : "inspect"; break
        case "openapp": motion = "launch"; break
        case "download": motion = "download"; break
        case "screenshot": motion = "snapshot"; break
        case "success": motion = "attention"; break
        case "error": motion = "shake"; break
        }
        if (animationCtrl) animationCtrl.play(motion)
    }
    function result(id, outcome, run) {
        if (id !== toolCallId) return
        if (run && run !== runId) return
        pendingResult = outcome
        if (Date.now() - startedAt >= 900) showResult()
    }
    function showResult() {
        phase = pendingResult
        pendingResult = ""
        var ok = phase === "done"
        if (expressionCtrl) expressionCtrl.flash(ok ? "happy" : "unsure", 900)
        if (animationCtrl) animationCtrl.play(ok ? "bounce" : "shake")
        if (moodCtrl) moodCtrl.setMood(ok ? "happy" : "concerned", 0.7, 3)
        clock.restart()
    }
    function complete(name) {
        if (name !== _currentScene) return
        var previous = _currentScene
        clock.stop()
        if (animationCtrl) animationCtrl.stopAction()
        _currentScene = ""; _scenePriority = 0; phase = ""
        toolCallId = ""; runId = ""
        if (gazeCtrl) gazeCtrl.rest(200)
        sceneFinished(previous)
    }
    function cancel() {
        if (!active) return
        var previous = _currentScene
        complete(previous)
        sceneCancelled(previous)
    }
    property Timer _clock: Timer {
        id: clock
        interval: 950
        repeat: true
        onTriggered: {
            if (ctrl.pendingResult) { ctrl.showResult(); return }
            if (ctrl.phase !== "working" && ctrl.phase !== "approval") { ctrl.complete(ctrl._currentScene); return }
            if (!ctrl.toolCallId && ctrl.step++ >= 2) { ctrl.complete(ctrl._currentScene); return }
            ctrl.animateWork()
        }
    }
}
