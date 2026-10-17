"""Own-process F3 lifecycle tests; --plasma adds real KWin observations.

No credentials, microphone, models, desktop key injection, or third-party windows.
"""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import sqlite3
import subprocess
import shutil
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def wait_for(work, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            result = work()
            if result:
                return result
        except (OSError, ValueError, urllib.error.URLError) as error:
            last = error
        time.sleep(0.05)
    raise AssertionError(f"timeout: {last}")


def children(pid):
    result = []
    for task in Path(f"/proc/{pid}/task").glob("*"):
        try:
            result.extend(int(p) for p in (task / "children").read_text().split())
        except FileNotFoundError:
            pass
    return sorted(set(result))


def bus(*args, env=None):
    return subprocess.check_output(["qdbus6", *map(str, args)], env=env, text=True, timeout=5).strip()


def sni_for_pid(pid, env):
    items = bus("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher.RegisteredStatusNotifierItems", env=env)
    for item in items.splitlines():
        service, separator, path = item.partition("/")
        owner_pid = bus("org.freedesktop.DBus", "/org/freedesktop/DBus",
            "org.freedesktop.DBus.GetConnectionUnixProcessID", service, env=env)
        if int(owner_pid) == pid:
            return service, "/"+path if separator else "/StatusNotifierItem"
    return None


def menu_action(pid, label_prefix, env):
    service, _ = wait_for(lambda: sni_for_pid(pid, env))
    layout = subprocess.check_output(["busctl", "--user", "--timeout=3", "call", service,
        "/MenuBar", "com.canonical.dbusmenu", "GetLayout", "iias", "--", "0", "-1", "0"], env=env, text=True, timeout=5)
    match = re.search(r'\(ia\{sv\}av\) (\d+) \d+ "label" s "'+re.escape(label_prefix)+r'[^\"]*"', layout)
    assert match, "Own DBusMenu action not found: "+label_prefix
    subprocess.run(["busctl", "--user", "--timeout=3", "call", service, "/MenuBar",
        "com.canonical.dbusmenu", "Event", "isvu", "--", match[1], "clicked", "i", "0", "0"],
        env=env, check=True, capture_output=True, timeout=5)


def metrics(pid):
    stat = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
    memory = Path(f"/proc/{pid}/smaps_rollup").read_text()
    return {"cpu_ticks": int(stat[11])+int(stat[12]),
            **{k.lower()+"_kib": int(re.search(rf"^{k}:\s+(\d+)", memory, re.M)[1]) for k in ("Rss", "Pss")}}


def launcher_test(binary, profile, env, name, output):
    gio = shutil.which("gio")
    if not gio:
        return {"name": "xdg_launcher", "status": "NOT_TESTED", "reason": "gio unavailable"}
    # A test-only desktop entry, not installed in the personal KDE profile.
    assert not any(c in str(binary) for c in '\\"%$`')
    desktop = profile / "data/applications/kda-f3-test.desktop"
    desktop.parent.mkdir(mode=0o700, exist_ok=True)
    desktop.write_text('[Desktop Entry]\nType=Application\nName=F3 isolated launcher\nTerminal=false\nExec="'+str(binary)+'" --no-shortcuts --http-port 0\n')
    process_id = None
    qml = None
    try:
        subprocess.run([gio, "launch", str(desktop)], env=env, cwd=profile, stdout=output, stderr=subprocess.STDOUT, check=True, timeout=8)
        def owner():
            result = subprocess.run(["qdbus6", "org.freedesktop.DBus", "/org/freedesktop/DBus",
                "org.freedesktop.DBus.GetConnectionUnixProcessID", name], env=env, capture_output=True, text=True, timeout=5)
            return int(result.stdout.strip()) if result.returncode == 0 else None
        candidate = wait_for(owner)
        wait_for(lambda: candidate in children(os.getpid()))
        proc = Path(f"/proc/{candidate}")
        assert (proc / "exe").resolve() == binary.resolve()
        selected_env = dict(part.split(b"=", 1) for part in (proc / "environ").read_bytes().split(b"\0") if b"=" in part)
        assert selected_env[b"XDG_CONFIG_HOME"].decode() == env["XDG_CONFIG_HOME"]
        process_id = candidate
        qml = wait_for(lambda: next((pid for pid in children(process_id) if b"/qml/Main.qml" in Path(f"/proc/{pid}/cmdline").read_bytes()), None))
        time.sleep(0.4)
        return {"name": "xdg_launcher", "status": "PASS", "category": "AUTOMATED_QML", "backend_pid": process_id, "qml_pid": qml}
    finally:
        if process_id is not None and Path(f"/proc/{process_id}").exists():
            fd = os.pidfd_open(process_id)
            signal.pidfd_send_signal(fd, signal.SIGTERM)
            os.close(fd)
            wait_for(lambda: Path(f"/proc/{process_id}/stat").read_text().split(") ",1)[1].startswith("Z") if Path(f"/proc/{process_id}").exists() else True, 15)
            try: os.waitpid(process_id, 0)
            except ChildProcessError: pass
        if qml is not None:
            assert not Path(f"/proc/{qml}").exists(), "launcher left its own QML alive"


def sample(pids, seconds=3):
    start = time.monotonic()
    before = {str(pid): metrics(pid) for pid in pids}
    time.sleep(seconds)
    after = {str(pid): metrics(pid) for pid in pids}
    elapsed = time.monotonic()-start
    for pid, value in after.items():
        value["cpu_one_core_percent"] = round((value["cpu_ticks"]-before[pid]["cpu_ticks"])/os.sysconf("SC_CLK_TCK")/elapsed*100, 2)
        del value["cpu_ticks"]
    return {"seconds": round(elapsed, 3), "processes": after}


def pending_consent_test(repo, profile, env, binary, output, plasma=False):
    """A mock requests a read; killing Qt must revoke it, never disclose/replay."""
    fixture = profile / "synthetic"
    fixture.mkdir(mode=0o700)
    (fixture / "read.txt").write_text("PRIVATE_SYNTHETIC_F3_CONTENT")
    calls = []
    class Provider(BaseHTTPRequestHandler):
        def log_message(self, *_): pass
        def do_POST(self):
            assert self.path == "/v1/chat/completions"
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            calls.append(body)
            offered = {t["function"]["name"] for t in body.get("tools", [])}
            assert offered == {"system_info", "read_file", "create_file", "edit_file"}, offered
            response = {"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "untrusted-provider-id",
                "type": "function", "function": {"name": "read_file", "arguments": json.dumps({"path": str(fixture / "read.txt")})}}]}, "finish_reason": "tool_calls"}]}
            data = ("data: "+json.dumps(response)+"\n\ndata: [DONE]\n\n").encode()
            self.send_response(200); self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data)
    provider = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    worker = threading.Thread(target=provider.serve_forever, daemon=True); worker.start()
    config = profile / "config/kde-assistant/config.json"
    cfg = json.loads(config.read_text())
    cfg["ai"].update(provider="custom", base_url=f"http://127.0.0.1:{provider.server_port}/v1", api_key="fixture-local",
        model="f3-synthetic", enable_tool_calling=True, provider_keys={}, provider_base_urls={}, provider_models={})
    for tool in ("system_info", "read_file", "create_file", "edit_file"):
        cfg["tools"][tool] = True
    cfg["tools"]["allowed_paths"] = [str(fixture)]
    config.write_text(json.dumps(cfg))
    resources = profile / "consent-resources"
    resources.mkdir(mode=0o700)
    shutil.copytree(repo / "qml", resources / "qml")
    shutil.copytree(repo / "assets", resources / "assets", ignore=shutil.ignore_patterns("models"))
    wrapper = resources / "qml/Main.qml"
    second_request = ('property int observedSessions: 0\nonCurrentSessionIdChanged: { if(currentSessionId) { observedSessions++; if(observedSessions===2) requestAgain.start() }}\n'
        'Timer { id: requestAgain; interval: 800; repeat: false; onTriggered: main.sendMessage("F3 second synthetic consent") }\n') if plasma else ''
    wrapper.write_text('import QtQuick\nimport '+json.dumps((repo / "qml").as_uri())+' as Original\nOriginal.Main {\n'
        'id: main\nTimer { interval: 800; running: true; repeat: false; onTriggered: main.sendMessage("F3 synthetic consent") }\n}\n')
    if second_request:
        wrapper.write_text(wrapper.read_text().rsplit('}', 1)[0]+second_request+'}\n')
    private_env = env.copy(); private_env["KDE_ASSISTANT_DATA_DIR"] = str(resources)
    app = None
    dbpath = profile / "data/kde-assistant/sessions.db"
    def state():
        with sqlite3.connect(dbpath) as db:
            return db.execute("select state from permission_requests order by rowid desc limit 1").fetchone()
    try:
        app = subprocess.Popen([str(binary), "--no-shortcuts", "--http-port", "0"], cwd=profile,
            env=private_env, stdout=output, stderr=subprocess.STDOUT)
        qml = wait_for(lambda: next((pid for pid in children(app.pid) if b"/qml/Main.qml" in Path(f"/proc/{pid}/cmdline").read_bytes()), None))
        wait_for(lambda: state() == ("pending",))
        wait_for(lambda: "F3_CONSENT_WINDOW" in (profile / "app.log").read_text())
        assert state() == ("pending",)
        assert len(calls) == 1
        if plasma:
            pending_snapshot = kwin(profile, qml)
            assert state() == ("pending",)
            (profile / "consent-kwin-pending.json").write_text(json.dumps(pending_snapshot, indent=2))
            menu_action(qml, "Nueva sesi", private_env)
            def first_revoked():
                with sqlite3.connect(dbpath) as db:
                    return db.execute("select state from permission_requests order by rowid limit 1").fetchone() == ("revoked",)
            wait_for(first_revoked)
            wait_for(lambda: len(calls) == 2 and state() == ("pending",))
            assert first_revoked()
        time.sleep(0.4)
        os.kill(qml, signal.SIGKILL); app.wait(timeout=15)
        assert app.returncode != 0
        assert state()[0] in ("revoked", "denied"), state()
        with sqlite3.connect(dbpath) as db:
            assert db.execute("select state from operation_journal order by rowid desc limit 1").fetchone()[0] in ("abandoned", "denied")
            assert not db.execute("select 1 from permission_requests where state in ('pending','approved')").fetchall()
        expected_calls = 2 if plasma else 1
        assert len(calls) == expected_calls and "PRIVATE_SYNTHETIC_F3_CONTENT" not in json.dumps(calls)
        # Restart the genuine UI, not the auto-requesting synthetic wrapper.
        app = subprocess.Popen([str(binary), "--no-shortcuts", "--http-port", "0"], cwd=profile,
            env=env, stdout=output, stderr=subprocess.STDOUT)
        qml = wait_for(lambda: next((pid for pid in children(app.pid) if b"/qml/Main.qml" in Path(f"/proc/{pid}/cmdline").read_bytes()), None))
        time.sleep(1)
        assert app.poll() is None and len(calls) == expected_calls
        if plasma: menu_action(qml, "Salir", env)
        else: app.terminate()
        app.wait(timeout=15)
        assert app.returncode == 0
        assert not Path(f"/proc/{qml}").exists()
        assert "F3_CONSENT_WINDOW" in (profile / "app.log").read_text()
        return {"name": "qml_loss_pending_consent_revoke_restart_no_replay", "status": "PASS", "category": "AUTOMATED_QML/AUTOMATED_KWIN" if plasma else "AUTOMATED_QML", "provider_calls": len(calls), "final_permission": state()[0], "tray_new_session_and_quit": "PASS" if plasma else "NOT_TESTED"}
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
            try: app.wait(timeout=12)
            except subprocess.TimeoutExpired: app.kill(); app.wait()
        provider.shutdown(); provider.server_close(); worker.join(timeout=3)


def kwin(root, pid, action="observe", caption="KDE Assistant v2", allow_missing=False):
    """Load/unload one uniquely named script, restricted to an owned PID/title."""
    name = "kda_f3_"+os.urandom(8).hex()
    marker = "F3_KWIN_"+name
    effects = {"observe": "", "minimize": "w.minimized = true;",
               "maximize": "w.setMaximize(true, true);", "close": "w.closeWindow();", "activate": "workspace.activeWindow=w;"}
    path = root / (name+".js")
    path.write_text("""var found=false; var list=workspace.windowList();
for(var i=0;i<list.length;i++) { var w=list[i];
if(w.pid === %d && w.caption === %s) {
found=true;
%s
var g=w.frameGeometry;
print(%s + JSON.stringify({found:true,pid:w.pid,active:w.active,minimized:w.minimized,at_ms:Date.now(),
stacking_index:workspace.stackingOrder ? workspace.stackingOrder.indexOf(w) : -1,
x:g.x,y:g.y,width:g.width,height:g.height,keepAbove:w.keepAbove}));
}}
if(!found) print(%s+JSON.stringify({found:false}));
""" % (pid, json.dumps(caption), effects[action], json.dumps(marker), json.dumps(marker)))
    start = time.time()-1
    loaded = False
    try:
        sid = bus("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.loadScript", path, name)
        assert int(sid) >= 0
        loaded = True
        # KWin 6 uses /Scripting/ScriptN; do not call Scripting.start globally.
        bus("org.kde.KWin", f"/Scripting/Script{sid}", "org.kde.kwin.Script.run")
        def observation():
            completed = subprocess.run(["journalctl", "--user", "--since", f"@{start:.3f}",
                "--grep", marker, "--no-pager", "-o", "cat", "-n", "3"], text=True, capture_output=True, timeout=5)
            assert completed.returncode in (0, 1), completed.stderr
            logs = completed.stdout
            for line in logs.splitlines():
                if marker in line:
                    return json.loads(line.split(marker, 1)[1])
            return None
        result = wait_for(observation, 4)
        return result if result.get("found") or allow_missing else None
    finally:
        if loaded:
            bus("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", name)
            assert bus("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.isScriptLoaded", name) == "false"


def runtime_tests(repo, root, base_env, plasma=False):
    env = base_env.copy()
    profile = root / "runtime"
    profile.mkdir(mode=0o700)
    for key, folder in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                        ("XDG_CACHE_HOME", "cache"), ("TMPDIR", "tmp")]:
        target = profile / folder
        target.mkdir(mode=0o700)
        env[key] = str(target)
    env["QT_QPA_PLATFORM"] = "wayland" if plasma else "offscreen"
    binary = repo / "target/debug/kde-assistant"
    logfile = profile / "app.log"
    output = logfile.open("w")
    app = None
    helper = None
    results = []
    # Reap our adopted Qt children after the explicit parent SIGKILL scenario.
    assert ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) == 0  # PR_SET_CHILD_SUBREAPER
    started = time.monotonic()
    try:
        app = subprocess.Popen([str(binary), "--no-shortcuts", "--http-port", "0"],
            cwd=profile, env=env, stdout=output, stderr=subprocess.STDOUT)
        def ready():
            assert app.poll() is None, logfile.read_text()[-2500:]
            match = re.search(r"texto listo en http://127.0.0.1:(\d+)", logfile.read_text())
            if not match:
                return None
            port = int(match[1])
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/api/health", timeout=1) as response:
                return port if response.status == 200 else None
        port = wait_for(ready)
        health_seconds = round(time.monotonic()-started, 3)
        qml = wait_for(lambda: next((pid for pid in children(app.pid)
            if b"/qml/Main.qml" in Path(f"/proc/{pid}/cmdline").read_bytes()), None))
        # PR_SET_PDEATHSIG follows the actual creator thread. Qt must be spawned
        # by the process leader, not a disposable Tokio/spawn_blocking worker.
        creator_threads = [int(task.name) for task in Path(f"/proc/{app.pid}/task").glob("*")
            if qml in [int(pid) for pid in (task / "children").read_text().split()]]
        assert creator_threads == [app.pid], creator_threads
        results.append({"name": "qml_created_by_main_thread", "status": "PASS", "category": "AUTOMATED_QML",
            "backend_pid": app.pid, "spawning_tid": creator_threads[0]})
        token = (profile / "config/kde-assistant/server.token").read_text().strip()
        def api(path, data=None):
            request = urllib.request.Request(f"http://127.0.0.1:{port}"+path, data=data,
                headers={"Authorization": "Bearer "+token, "Content-Type": "application/json"})
            with urllib.request.urlopen(request, timeout=3) as response:
                return json.load(response)
        name = "org.kde.assistant.p"+hashlib.sha256(str((profile / "config/kde-assistant").resolve()).encode()).hexdigest()
        # Wait for the outbound channel; no queued/replayed activation.
        def activate():
            result = subprocess.run(["qdbus6", name, "/org/kde/assistant/Desktop", "org.kde.assistant.Desktop1.Activate"],
                env=env, capture_output=True, text=True, timeout=5)
            return result.returncode == 0
        wait_for(activate)
        results.append({"name": "startup_other_cwd", "status": "PASS", "category": "AUTOMATED_KWIN" if plasma else "AUTOMATED_QML",
                        "health_seconds": health_seconds, "ui_control_seconds": round(time.monotonic()-started, 3), "backend_pid": app.pid, "qml_pid": qml, "port": port})
        tools = api("/api/tools")
        (profile / "catalog.json").write_text(json.dumps(tools))
        duplicate = subprocess.run([str(binary), "--no-shortcuts", "--http-port", "0"], cwd=repo,
            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=10)
        assert duplicate.returncode == 0, duplicate.stdout
        assert children(app.pid) == [qml]
        assert api("/api/desktop/status")["shortcuts"] == "disabled"
        results.append({"name": "single_instance_activation", "status": "PASS", "category": "AUTOMATED_QML"})
        # A second private XDG profile must coexist, not activate the first.
        second = env.copy()
        for key in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "TMPDIR"):
            directory = Path(env[key]).parent / (Path(env[key]).name+"-second")
            directory.mkdir(mode=0o700)
            second[key] = str(directory)
        with (profile / "second.log").open("w") as other_log:
            other = subprocess.Popen([str(binary), "--ui-off", "--no-shortcuts", "--http-port", "0"], env=second, cwd=profile, stdout=other_log, stderr=subprocess.STDOUT)
            try:
                wait_for(lambda: "texto listo" in (profile / "second.log").read_text())
                assert other.poll() is None and app.poll() is None
            finally:
                other.terminate(); other.wait(timeout=12)
        results.append({"name": "independent_profiles", "status": "PASS", "category": "AUTOMATED_QML"})
        if plasma:
            initial = wait_for(lambda: kwin(profile, qml))
            assert initial["width"] > 0 and initial["height"] > 0
            tray, tray_path = wait_for(lambda: sni_for_pid(qml, env))
            menu = subprocess.check_output(["busctl", "--user", "--timeout=3", "get-property", tray,
                tray_path, "org.kde.StatusNotifierItem", "Menu"], env=env, text=True, timeout=5).strip()
            assert menu == 'o "/MenuBar"', repr(menu)
            layout = subprocess.check_output(["busctl", "--user", "--timeout=3", "call", tray, "/MenuBar", "com.canonical.dbusmenu", "GetLayout", "iias", "--", "0", "-1", "0"], env=env, text=True, timeout=5)
            (profile / "dbusmenu-layout.txt").write_text(layout)
            # busctl's textual formatter escapes non-ASCII UTF-8 bytes.
            assert "Nueva sesi" in layout and "Salir" in layout, layout
            def click():
                bus(tray, tray_path, "org.kde.StatusNotifierItem.Activate", "0", "0", env=env)
            # Synthetic DBus Activate carries no real user-input token. Set only
            # our own window active to establish the active->hide precondition.
            kwin(profile, qml, "activate")
            wait_for(lambda: kwin(profile, qml)["active"])
            time.sleep(0.4)  # let Qt receive the compositor activation event
            click(); time.sleep(0.4)
            assert not kwin(profile, qml, allow_missing=True)["found"], "Active own window did not hide"
            click(); time.sleep(0.4)
            restored = kwin(profile, qml)
            assert not restored["minimized"], restored
            kwin(profile, qml, "minimize"); time.sleep(0.3)
            assert kwin(profile, qml)["minimized"]
            click(); time.sleep(0.5)
            restored = kwin(profile, qml)
            assert not restored["minimized"], restored
            results.append({"name": "tray_dbusmenu_minimize_restore_one_activation", "status": "PASS", "category": "AUTOMATED_KWIN", "state": restored})
            kwin(profile, qml, "maximize"); time.sleep(0.4)
            before = kwin(profile, qml)
            assert before["width"] > initial["width"] or before["height"] > initial["height"], "maximize precondition did not change geometry"
            kwin(profile, qml, "activate")
            wait_for(lambda: kwin(profile, qml)["active"])
            time.sleep(0.4)
            click(); time.sleep(0.3); click(); time.sleep(0.5)
            after = kwin(profile, qml)
            assert before["width"] == after["width"] and before["height"] == after["height"], (before, after)
            results.append({"name": "maximized_state_preserved", "status": "PASS", "category": "AUTOMATED_KWIN", "state": after})
            # Only another window we launch; no other desktop application is touched.
            helper_qml = profile / "FocusHelper.qml"
            helper_qml.write_text('import QtQuick\nWindow { title: "F3 own focus helper"; visible: true; width: 200; height: 120; Component.onCompleted: requestActivate() }')
            helper = subprocess.Popen(["qml6", str(helper_qml)], env=env, stdout=output, stderr=subprocess.STDOUT)
            time.sleep(1)
            kwin(profile, helper.pid, "activate", caption="F3 own focus helper")
            time.sleep(0.3)
            unfocused = kwin(profile, qml)
            assert not unfocused["active"], unfocused
            click(); time.sleep(0.5)
            focus = kwin(profile, qml)
            results.append({"name": "unfocused_synthetic_activation", "status": "PASS" if focus["active"] else "BLOCKED",
                "category": "AUTOMATED_KWIN", "state": focus,
                "limitation": "Synthetic SNI call has no real input token; human tray/focus verification remains NOT_TESTED"})
            helper.terminate(); helper.wait(timeout=5); helper = None
            # Close of own window maps to tray; it must restore with one Activate.
            kwin(profile, qml, "close"); time.sleep(0.4)
            assert app.poll() is None
            click(); time.sleep(0.5)
            assert not kwin(profile, qml)["minimized"]
            results.append({"name": "close_to_tray_restore", "status": "PASS", "category": "AUTOMATED_KWIN"})
        # Samples, not universal budgets; keep compiler work out of these intervals.
        if plasma:
            kwin(profile, qml, "activate")
            wait_for(lambda: kwin(profile, qml)["active"])
            time.sleep(0.4)
        time.sleep(2)
        perf = {"visible_idle": sample([app.pid, qml])}
        subprocess.run([str(binary), "--toggle"], env=env, cwd=repo, check=True, stdout=output, stderr=subprocess.STDOUT, timeout=10)
        time.sleep(0.5)
        perf["hidden_idle"] = sample([app.pid, qml])
        if plasma:
            assert not kwin(profile, qml, allow_missing=True)["found"], "hidden sample was not hidden"
        if plasma:
            click(); time.sleep(0.4)
            before = {str(pid): metrics(pid) for pid in (app.pid, qml)}
            cycle_start = time.monotonic()
            for _ in range(5):
                kwin(profile, qml, "activate")
                wait_for(lambda: kwin(profile, qml)["active"])
                time.sleep(0.4)
                click(); time.sleep(0.2)
                assert not kwin(profile, qml, allow_missing=True)["found"]
                click(); time.sleep(0.2)
                assert not kwin(profile, qml)["minimized"]
            elapsed = time.monotonic()-cycle_start
            after = {str(pid): metrics(pid) for pid in (app.pid, qml)}
            for pid, value in after.items():
                value["cpu_one_core_percent"] = round((value["cpu_ticks"]-before[pid]["cpu_ticks"])/os.sysconf("SC_CLK_TCK")/elapsed*100, 2)
                del value["cpu_ticks"]
            perf["five_open_close_cycles"] = {"seconds": round(elapsed,3), "processes": after}
            assert not kwin(profile, qml)["minimized"]
        (profile / "performance.json").write_text(json.dumps(perf, indent=2))
        results.append({"name": "resource_samples", "category": "AUTOMATED_KWIN" if plasma else "AUTOMATED_QML", "status": "PASS", "samples": perf})
        # Child failure shuts down the controller; it never replays a tool turn.
        os.kill(qml, signal.SIGKILL)
        app.wait(timeout=15)
        assert app.returncode != 0
        assert not Path(f"/proc/{qml}").exists()
        assert not children(app.pid)
        app = None
        with socket.socket() as probe:
            probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            probe.bind(("127.0.0.1", port))
        results.append({"name": "qml_failure_owned_cleanup_socket_release", "status": "PASS", "category": "AUTOMATED_QML"})
        # Lock file persists but ownership is released after crash/normal shutdown.
        app = subprocess.Popen([str(binary), "--no-shortcuts", "--http-port", "0"], cwd=repo, env=env, stdout=output, stderr=subprocess.STDOUT)
        qml2 = wait_for(lambda: next((pid for pid in children(app.pid) if b"/qml/Main.qml" in Path(f"/proc/{pid}/cmdline").read_bytes()), None))
        os.kill(app.pid, signal.SIGKILL); app.wait(timeout=5); app = None
        # prctl parent-death signal kills Qt; subreaper collects our adopted child.
        wait_for(lambda: Path(f"/proc/{qml2}/stat").read_text().split(") ", 1)[1].startswith("Z") if Path(f"/proc/{qml2}").exists() else True, 5)
        try: os.waitpid(qml2, 0)
        except ChildProcessError: pass
        assert not Path(f"/proc/{qml2}").exists()
        results.append({"name": "parent_death_kills_own_qml", "status": "PASS", "category": "AUTOMATED_QML"})
        results.append(launcher_test(binary, profile, env, name, output))
        # Missing resources must fail explicitly, rather than picking the CWD.
        bad = env.copy(); bad["KDE_ASSISTANT_DATA_DIR"] = str(profile / "missing")
        invalid = subprocess.run([str(binary), "--no-shortcuts", "--http-port", "0"], cwd=repo, env=bad, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=15)
        assert invalid.returncode != 0 and "Faltan recursos" in invalid.stdout, invalid.stdout
        results.append({"name": "missing_assets_fail_explicitly", "status": "PASS", "category": "AUTOMATED_QML"})
        results.append(pending_consent_test(repo, profile, env, binary, output, plasma=plasma))
        db = sqlite3.connect(profile / "data/kde-assistant/sessions.db")
        assert db.execute("pragma integrity_check").fetchone()[0] == "ok"
        assert not db.execute("pragma foreign_key_check").fetchall(); db.close()
        assert not re.search(r"ReferenceError:|TypeError:|Cannot assign to non-existent property", logfile.read_text()), "QML runtime error; see private app.log"
        return results
    finally:
        for process in (helper, app):
            if process is not None and process.poll() is None:
                process.terminate()
                try: process.wait(timeout=12)
                except subprocess.TimeoutExpired: process.kill(); process.wait()
        output.close()
        (profile / "results.json").write_text(json.dumps(results, indent=2))
