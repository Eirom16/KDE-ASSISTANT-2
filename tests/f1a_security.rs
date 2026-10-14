//! Regresiones F1A: sólo fixtures sintéticos en directorios temporales privados.
use kde_assistant_lib::{
    backend::tool_executor::{ToolCtx, ToolExecutor},
    models::{Config, ToolCall, ToolResult},
};
use std::os::unix::fs::{symlink, DirBuilderExt};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;

struct Fixture {
    base: PathBuf,
    allowed: PathBuf,
    outside: PathBuf,
    config: Arc<RwLock<Config>>,
    executor: ToolExecutor,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("kda-f1a-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        std::fs::create_dir(&allowed).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let mut config = Config::default();
        config.tools.allowed_paths = vec![allowed.display().to_string()];
        let config = Arc::new(RwLock::new(config));
        let executor = ToolExecutor::legacy_fixture(config.clone());
        Self {
            base,
            allowed,
            outside,
            config,
            executor,
        }
    }
    async fn run(&self, name: &str, args: serde_json::Value) -> ToolResult {
        self.executor
            .execute(&call(name, args), &ToolCtx::auto(None))
            .await
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.base).unwrap();
    }
}
fn call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "f1a-fixture".into(),
        name: name.into(),
        arguments: serde_json::from_value(args).unwrap(),
    }
}
fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn search_existing_roots_survive_missing_roots() {
    let f = Fixture::new();
    let second = f.base.join("second");
    std::fs::create_dir(&second).unwrap();
    std::fs::write(f.allowed.join("fixture-first.txt"), "first").unwrap();
    std::fs::write(second.join("fixture-second.md"), "second").unwrap();
    f.config.write().await.tools.allowed_paths = vec![
        f.base.join("missing").display().to_string(),
        f.allowed.display().to_string(),
        f.allowed.join("missing-child").display().to_string(),
        second.display().to_string(),
    ];
    for name in ["find_file", "find_document"] {
        let r = f.run(name, serde_json::json!({"query": "fixture"})).await;
        assert!(r.success, "{name}: {}", r.content);
        assert!(r.content.contains("fixture-first.txt"));
        assert!(r.content.contains("fixture-second.md"));
    }
}

#[tokio::test]
async fn search_security_errors_are_not_missing_roots() {
    let f = Fixture::new();
    std::fs::write(f.allowed.join("fixture.txt"), "inside").unwrap();
    std::fs::write(f.outside.join("fixture-secret.txt"), "outside").unwrap();
    let link = f.base.join("linked-root");
    let dangling = f.base.join("dangling-root");
    let file = f.base.join("file-root");
    symlink(&f.outside, &link).unwrap();
    symlink(f.base.join("absent-target"), &dangling).unwrap();
    std::fs::write(&file, "not a directory").unwrap();
    for invalid in [link, dangling, file, PathBuf::from("relative-root")] {
        f.config.write().await.tools.allowed_paths = vec![
            f.allowed.display().to_string(),
            invalid.display().to_string(),
        ];
        for name in ["find_file", "find_document"] {
            let r = f.run(name, serde_json::json!({"query": "fixture"})).await;
            assert!(!r.success, "{name}: {}", r.content);
            assert!(
                r.content.contains("insegura o inaccesible"),
                "{}",
                r.content
            );
            assert!(!r.content.contains("fixture-secret.txt"));
        }
    }
}

#[tokio::test]
async fn search_explicit_invalid_directory_is_never_skipped() {
    let f = Fixture::new();
    std::fs::write(f.allowed.join("fixture.txt"), "inside").unwrap();
    let link = f.allowed.join("linked-dir");
    symlink(&f.outside, &link).unwrap();
    for directory in [
        f.allowed.join("missing"),
        link,
        f.outside.clone(),
        f.allowed.join("../outside"),
        PathBuf::from("relative-dir"),
    ] {
        for name in ["find_file", "find_document"] {
            let r = f
                .run(
                    name,
                    serde_json::json!({"query": "fixture", "dir": directory}),
                )
                .await;
            assert!(!r.success, "{name}: {}", r.content);
            assert!(!r.content.contains("fixture.txt"));
        }
    }
}

#[tokio::test]
async fn search_without_usable_roots_returns_clear_error() {
    let f = Fixture::new();
    for roots in [vec![], vec![f.base.join("missing").display().to_string()]] {
        f.config.write().await.tools.allowed_paths = roots;
        for name in ["find_file", "find_document"] {
            let r = f.run(name, serde_json::json!({"query": "fixture"})).await;
            assert!(!r.success);
            assert!(
                r.content.contains("ninguna raíz utilizable"),
                "{}",
                r.content
            );
        }
    }
}

#[test]
#[ignore = "requiere unshare user/mount y mount; sólo fixtures privados en un namespace hijo"]
fn search_mount_boundaries_remain_closed() {
    // La invocación externa siempre entra primero en un namespace privado.
    // El marcador sólo evita recursión al reejecutar este mismo test en el hijo.
    const CHILD: &str = "KDA_F1A_MOUNT_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
            ])
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "search_mount_boundaries_remain_closed",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(
                "KDA_F1A_PARENT_MOUNT_NS",
                std::fs::read_link("/proc/self/ns/mnt").unwrap(),
            )
            .status()
            .unwrap();
        assert!(status.success(), "namespace de montaje no disponible");
        return;
    }
    assert_ne!(
        std::fs::read_link("/proc/self/ns/mnt").unwrap(),
        PathBuf::from(std::env::var_os("KDA_F1A_PARENT_MOUNT_NS").unwrap()),
        "el hijo debe tener un namespace de montaje distinto"
    );
    let f = Fixture::new();
    let mounted = f.allowed.join("mounted");
    std::fs::create_dir(&mounted).unwrap();
    std::fs::write(f.allowed.join("fixture-inside.txt"), "inside").unwrap();
    std::fs::write(f.outside.join("fixture-mounted.txt"), "mounted").unwrap();
    let status = std::process::Command::new("mount")
        .arg("--bind")
        .arg(&f.outside)
        .arg(&mounted)
        .status()
        .unwrap();
    assert!(status.success());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.block_on(async {
            f.config.write().await.tools.allowed_paths = vec![
                f.base.join("missing").display().to_string(),
                f.allowed.display().to_string(),
            ];
            for name in ["find_file", "find_document"] {
                let r = f.run(name, serde_json::json!({"query": "fixture"})).await;
                assert!(r.success, "{}", r.content);
                assert!(r.content.contains("fixture-inside.txt"));
                assert!(!r.content.contains("fixture-mounted.txt"));
                let r = f
                    .run(
                        name,
                        serde_json::json!({"query": "fixture", "dir": mounted}),
                    )
                    .await;
                assert!(!r.success, "cruzó montaje: {}", r.content);
            }
            f.config.write().await.tools.allowed_paths = vec![
                f.base.join("missing").display().to_string(),
                mounted.display().to_string(),
            ];
            for name in ["find_file", "find_document"] {
                let r = f.run(name, serde_json::json!({"query": "fixture"})).await;
                assert!(
                    r.success && r.content.contains("fixture-mounted.txt"),
                    "{}",
                    r.content
                );
            }
        });
    }));
    assert!(std::process::Command::new("umount")
        .arg(&mounted)
        .status()
        .unwrap()
        .success());
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[tokio::test]
async fn disabled_tool_with_and_without_write_lock() {
    let f = Fixture::new();
    f.config.write().await.tools.notify = false;
    let r = f.run("notify", serde_json::json!({})).await;
    assert!(
        !r.success && r.content.contains("deshabilitado"),
        "{}",
        r.content
    );
    let guard = f.config.write().await;
    let r = f.run("notify", serde_json::json!({})).await;
    drop(guard);
    assert!(
        !r.success
            && (r.content.contains("deshabilitado")
                || r.content.contains("configuracion no disponible")),
        "{}",
        r.content
    );
}

#[tokio::test]
async fn dangling_symlink_creation_has_no_outside_effect() {
    let f = Fixture::new();
    let outside = f.outside.join("new.txt");
    let path = f.allowed.join("dangling.txt");
    symlink(&outside, &path).unwrap();
    let r = f
        .run(
            "create_file",
            serde_json::json!({"path": path, "content": "fixture"}),
        )
        .await;
    assert!(
        !outside.exists(),
        "creó archivo exterior: success={}",
        r.success
    );
    assert!(!r.success);
}

#[tokio::test]
async fn copy_derived_destination_symlink_has_no_outside_effect() {
    let f = Fixture::new();
    let source = f.allowed.join("source.txt");
    let dest = f.allowed.join("dest");
    let outside = f.outside.join("victim.txt");
    std::fs::write(&source, "copy fixture").unwrap();
    std::fs::write(&outside, "original").unwrap();
    std::fs::create_dir(&dest).unwrap();
    symlink(&outside, dest.join("source.txt")).unwrap();
    let r = f
        .run(
            "copy_file",
            serde_json::json!({"source": source, "destination": dest, "overwrite": true}),
        )
        .await;
    assert_eq!(text(&outside), "original", "success={}", r.success);
    assert!(!r.success);
}

#[tokio::test]
async fn malicious_backup_symlink_leaves_main_and_outside_unchanged() {
    let f = Fixture::new();
    let path = f.allowed.join("edit.txt");
    let outside = f.outside.join("victim.txt");
    std::fs::write(&path, "main original").unwrap();
    std::fs::write(&outside, "outside original").unwrap();
    symlink(&outside, path.with_extension("bak")).unwrap();
    let r = f
        .run(
            "edit_file",
            serde_json::json!({"path": path, "mode": "overwrite", "content": "changed"}),
        )
        .await;
    assert_eq!(text(&outside), "outside original", "success={}", r.success);
    assert_eq!(text(&path), "main original");
    assert!(!r.success);
}

#[tokio::test]
async fn all_public_tool_dispatches_respect_disabled_flags_and_contention() {
    let f = Fixture::new();
    let mut value = serde_json::to_value(Config::default()).unwrap();
    value["tools"] = serde_json::json!({});
    *f.config.write().await = serde_json::from_value(value).unwrap();
    for tool in kde_assistant_lib::backend::tool_registry::all_tools() {
        let r = f.run(&tool.function.name, serde_json::json!({})).await;
        assert!(
            !r.success && r.content.contains("deshabilitado"),
            "{}: {}",
            tool.function.name,
            r.content
        );
        let guard = f.config.write().await;
        let r = f.run(&tool.function.name, serde_json::json!({})).await;
        drop(guard);
        assert!(
            !r.success && r.content.contains("configuracion no disponible"),
            "{}: {}",
            tool.function.name,
            r.content
        );
    }
    assert_eq!(std::fs::read_dir(&f.allowed).unwrap().count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_updates_never_mix_tool_flag_and_custom_allowlist() {
    let f = Fixture::new();
    let mut enabled = f.config.read().await.clone();
    enabled.tools.allowed_paths = vec![f.allowed.display().to_string()];
    let mut disabled = enabled.clone();
    disabled.tools.create_file = false;
    disabled.tools.allowed_paths = vec![f.outside.display().to_string()];
    let config = f.config.clone();
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let writer_barrier = barrier.clone();
    let writer = tokio::spawn(async move {
        writer_barrier.wait().await;
        for _ in 0..500 {
            *config.write().await = disabled.clone();
            tokio::task::yield_now().await;
            *config.write().await = enabled.clone();
            tokio::task::yield_now().await;
        }
    });
    barrier.wait().await;
    for i in 0..500 {
        // A permite la herramienta pero prohíbe la ruta; B permite la ruta pero
        // deshabilita la herramienta. Ninguna versión autoriza esta escritura.
        let r = f.run("create_file", serde_json::json!({"path": f.outside.join(format!("{i}.txt")), "content": "fixture"})).await;
        assert!(!r.success, "mezcló versiones: {}", r.content);
        tokio::task::yield_now().await;
    }
    writer.await.unwrap();
    assert_eq!(std::fs::read_dir(&f.outside).unwrap().count(), 0);
}

#[tokio::test]
async fn legitimate_create_read_edit_and_copy() {
    let f = Fixture::new();
    let path = f.allowed.join("nested/deeper/file.txt");
    let r = f
        .run(
            "create_file",
            serde_json::json!({"path": path, "content": "original"}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    let r = f.run("read_file", serde_json::json!({"path": path})).await;
    assert!(r.success && r.content == "original");
    let r = f
        .run(
            "edit_file",
            serde_json::json!({"path": path, "mode": "append", "content": " appended"}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    let r = f
        .run(
            "edit_file",
            serde_json::json!({"path": path, "mode": "overwrite", "content": "updated"}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    assert_eq!(text(&path.with_extension("bak")), "original appended");
    let destination = f.allowed.join("new/deep/copy.txt");
    let r = f
        .run(
            "copy_file",
            serde_json::json!({"source": path, "destination": destination}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    assert_eq!(text(&destination), "updated");
    let r = f
        .run(
            "copy_file",
            serde_json::json!({"source": path, "destination": destination}),
        )
        .await;
    assert!(!r.success);
    let r = f
        .run(
            "copy_file",
            serde_json::json!({"source": path, "destination": destination, "overwrite": true}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    std::fs::create_dir(f.allowed.join("dest")).unwrap();
    let r = f
        .run(
            "copy_file",
            serde_json::json!({"source": path, "destination": f.allowed.join("dest")}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    assert_eq!(text(&f.allowed.join("dest/file.txt")), "updated");
    let r = f
        .run(
            "find_file",
            serde_json::json!({"query": "file", "dir": f.allowed}),
        )
        .await;
    assert!(r.success && r.content.contains("file.txt"), "{}", r.content);
}

#[tokio::test]
async fn outside_operations_and_invalid_edit_have_no_effects() {
    let f = Fixture::new();
    let path = f.outside.join("victim.txt");
    let inside = f.allowed.join("source.txt");
    std::fs::write(&path, "outside").unwrap();
    std::fs::write(&inside, "inside").unwrap();
    for (name, args) in [
        (
            "create_file",
            serde_json::json!({"path": f.outside.join("nested/new.txt"), "content": "new"}),
        ),
        ("read_file", serde_json::json!({"path": path})),
        (
            "edit_file",
            serde_json::json!({"path": path, "mode": "overwrite", "content": "changed"}),
        ),
        (
            "copy_file",
            serde_json::json!({"source": inside, "destination": path, "overwrite": true}),
        ),
        (
            "copy_file",
            serde_json::json!({"source": path, "destination": f.allowed.join("copy.txt")}),
        ),
        (
            "edit_file",
            serde_json::json!({"path": inside, "mode": "invalid", "content": "changed"}),
        ),
        (
            "create_file",
            serde_json::json!({"path": f.allowed.join("../outside/new.txt"), "content": "changed"}),
        ),
    ] {
        let r = f.run(name, args).await;
        assert!(!r.success, "{name}: {}", r.content);
    }
    assert_eq!(text(&path), "outside");
    assert_eq!(text(&inside), "inside");
    assert!(!f.outside.join("nested").exists());
    assert!(!inside.with_extension("bak").exists());
    assert!(!f.allowed.join("copy.txt").exists());
}

#[tokio::test]
async fn intermediate_symlinks_empty_allowlist_and_hardlinks_are_denied() {
    let f = Fixture::new();
    let victim = f.outside.join("victim.txt");
    std::fs::write(&victim, "original").unwrap();
    symlink(&f.outside, f.allowed.join("escape")).unwrap();
    std::fs::hard_link(&victim, f.allowed.join("hard.txt")).unwrap();
    for path in [
        f.allowed.join("escape/victim.txt"),
        f.allowed.join("hard.txt"),
    ] {
        for (name, args) in [
            ("read_file", serde_json::json!({"path": path})),
            (
                "edit_file",
                serde_json::json!({"path": path, "mode": "append", "content": "changed"}),
            ),
        ] {
            assert!(!f.run(name, args).await.success);
        }
    }
    assert!(
        !f.run(
            "create_file",
            serde_json::json!({"path": f.allowed.join("escape/new.txt"), "content": "changed"})
        )
        .await
        .success
    );
    f.config.write().await.tools.allowed_paths.clear();
    assert!(
        !f.run(
            "create_file",
            serde_json::json!({"path": f.allowed.join("new.txt"), "content": "changed"})
        )
        .await
        .success
    );
    assert_eq!(text(&victim), "original");
    assert!(!f.outside.join("new.txt").exists());
    assert!(!f.allowed.join("new.txt").exists());
}

#[tokio::test]
async fn disabled_llm_catalog_preserves_explicit_manual_file_flag() {
    let f = Fixture::new();
    f.config.write().await.ai.enable_tool_calling = false;
    let path = f.allowed.join("manual.txt");
    let r = f
        .run(
            "create_file",
            serde_json::json!({"path": path, "content": "manual fixture"}),
        )
        .await;
    assert!(r.success, "{}", r.content);
    f.config.write().await.tools.create_file = false;
    let r = f
        .run(
            "create_file",
            serde_json::json!({"path": f.allowed.join("denied.txt"), "content": "fixture"}),
        )
        .await;
    assert!(!r.success);
    assert!(!f.allowed.join("denied.txt").exists());
}

#[tokio::test]
async fn pathname_delegation_is_contained_until_f2() {
    let f = Fixture::new();
    let path = f.allowed.join("file.txt");
    std::fs::write(&path, "fixture").unwrap();
    for (name, args) in [
        ("open_file", serde_json::json!({"path": path})),
        (
            "kdeconnect",
            serde_json::json!({"action": "share_file", "path": path, "device": "synthetic"}),
        ),
    ] {
        let r = f.run(name, args).await;
        assert!(!r.success && r.content.contains("F2"), "{}", r.content);
    }
    assert_eq!(text(&path), "fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_symlink_replacement_never_writes_outside() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let f = Fixture::new();
    let source = f.allowed.join("source.txt");
    let edit = f.allowed.join("edit.txt");
    let create = f.allowed.join("race-create.txt");
    let dest = f.allowed.join("dest");
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(&source, "synthetic source").unwrap();
    std::fs::write(&edit, "synthetic edit").unwrap();
    let victim = f.outside.join("victim.txt");
    let absent = f.outside.join("absent.txt");
    std::fs::write(&victim, "untouched").unwrap();
    let targets = vec![
        (create.clone(), absent.clone()),
        (dest.join("source.txt"), victim.clone()),
        (edit.with_extension("bak"), victim.clone()),
        (edit.clone(), victim.clone()),
    ];
    let stop = Arc::new(AtomicBool::new(false));
    let cycles = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(std::sync::Barrier::new(2));
    let attacker_stop = stop.clone();
    let attacker_cycles = cycles.clone();
    let attacker_ready = ready.clone();
    let attacker = std::thread::spawn(move || {
        attacker_ready.wait();
        while !attacker_stop.load(Ordering::Acquire) {
            for (link, outside) in &targets {
                let _ = std::fs::remove_file(link);
                let _ = symlink(outside, link);
                std::thread::yield_now();
                let _ = std::fs::remove_file(link);
                let _ = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(link);
            }
            attacker_cycles.fetch_add(1, Ordering::Release);
        }
    });
    ready.wait();
    for _ in 0..150 {
        // Éxito o rechazo son válidos; nunca una escritura a través del link.
        let _ = f
            .run(
                "create_file",
                serde_json::json!({"path": create, "content": "created"}),
            )
            .await;
        let _ = f
            .run(
                "copy_file",
                serde_json::json!({"source": source, "destination": dest, "overwrite": true}),
            )
            .await;
        let _ = f
            .run(
                "edit_file",
                serde_json::json!({"path": edit, "mode": "overwrite", "content": "edited"}),
            )
            .await;
    }
    stop.store(true, Ordering::Release);
    attacker.join().unwrap();
    assert!(cycles.load(Ordering::Acquire) > 0);
    assert_eq!(text(&victim), "untouched");
    assert!(!absent.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_parent_directory_symlinks_do_not_escape() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let f = Fixture::new();
    let directory = f.allowed.join("race-dir");
    let parked = f.allowed.join("parked");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("secret.txt"), "inside fixture").unwrap();
    std::fs::write(f.outside.join("secret.txt"), "outside fixture").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let cycles = Arc::new(AtomicUsize::new(0));
    let attacker_stop = stop.clone();
    let attacker_cycles = cycles.clone();
    let outside = f.outside.clone();
    let attacker_dir = directory.clone();
    let attacker = std::thread::spawn(move || {
        use rustix::fs::{renameat_with, RenameFlags, CWD};
        symlink(&outside, &parked).unwrap();
        while !attacker_stop.load(Ordering::Acquire) {
            // EXCHANGE mantiene ambas entradas presentes: directorio <-> link.
            renameat_with(CWD, &attacker_dir, CWD, &parked, RenameFlags::EXCHANGE).unwrap();
            std::thread::yield_now();
            renameat_with(CWD, &attacker_dir, CWD, &parked, RenameFlags::EXCHANGE).unwrap();
            attacker_cycles.fetch_add(1, Ordering::Release);
        }
    });
    let mut bad_read = false;
    for i in 0..150 {
        let _ = f.run("create_file", serde_json::json!({"path": directory.join(format!("new-{i}.txt")), "content": "created"})).await;
        let r = f
            .run(
                "read_file",
                serde_json::json!({"path": directory.join("secret.txt")}),
            )
            .await;
        bad_read |= r.success && r.content != "inside fixture";
    }
    stop.store(true, Ordering::Release);
    attacker.join().unwrap();
    assert!(cycles.load(Ordering::Acquire) > 0);
    assert!(!bad_read);
    assert_eq!(std::fs::read_dir(&f.outside).unwrap().count(), 1);
    assert_eq!(text(&f.outside.join("secret.txt")), "outside fixture");
}

#[tokio::test]
async fn failed_append_decode_and_backup_collision_preserve_main() {
    let f = Fixture::new();
    let path = f.allowed.join("binary.txt");
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    let r = f
        .run(
            "edit_file",
            serde_json::json!({"path": path, "mode": "append", "content": "changed"}),
        )
        .await;
    assert!(!r.success);
    assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0xfe]);
    let path = f.allowed.join("main.bak");
    std::fs::write(&path, "original").unwrap();
    let r = f
        .run(
            "edit_file",
            serde_json::json!({"path": path, "mode": "overwrite", "content": "changed"}),
        )
        .await;
    assert!(!r.success);
    assert_eq!(text(&path), "original");
}

// Se reejecuta aisladamente bajo seccomp para comprobar ENOSYS sin introducir
// inyección de fallos ni fallbacks en el código de producción.
#[tokio::test]
async fn unavailable_kernel_primitives_fail_closed() {
    let f = Fixture::new();
    let path = f.allowed.join("main.txt");
    let created = f.allowed.join("created.txt");
    let copied = f.allowed.join("copied.txt");
    std::fs::write(&path, "original").unwrap();
    let denial = std::env::var("KDA_F1A_TEST_DENY_SYSCALL").unwrap_or_default();
    let create = f
        .run(
            "create_file",
            serde_json::json!({"path": created, "content": "new"}),
        )
        .await;
    let copy = f
        .run(
            "copy_file",
            serde_json::json!({"source": path, "destination": copied}),
        )
        .await;
    let read = f.run("read_file", serde_json::json!({"path": path})).await;
    let edit = f
        .run(
            "edit_file",
            serde_json::json!({"path": path, "mode": "overwrite", "content": "edited"}),
        )
        .await;
    if denial.is_empty() {
        assert!(create.success && copy.success && read.success && edit.success);
        assert_eq!(text(&path), "edited");
    } else {
        assert!(matches!(denial.as_str(), "openat2" | "renameat2"));
        assert!(!create.success && !copy.success && !edit.success);
        assert_eq!(read.success, denial == "renameat2");
        assert!(!created.exists() && !copied.exists() && !path.with_extension("bak").exists());
        assert_eq!(text(&path), "original");
        assert_eq!(std::fs::read_dir(&f.allowed).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(&f.outside).unwrap().count(), 0);
    }
}
