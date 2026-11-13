use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "dftwork-cli-contract-{}-{name}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("创建测试目录");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn failure_details(output: &Output) -> String {
    format!(
        "status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[cfg(windows)]
fn write_mock_hermes(dir: &Path) -> PathBuf {
    let path = dir.join("mock hermes.cmd");
    std::fs::write(
        &path,
        r#"@echo off
setlocal
set "ARGS=%*"
if "%~1"=="--version" (
  echo Hermes Agent v0.0.0-test
  exit /b 0
)
set "QUERY="
:args
if "%~1"=="" goto run
if "%~1"=="--query-file" set "QUERY=%~2"
shift
goto args
:run
>>"%DFTWORK_TEST_CAPTURE%" echo cwd=%CD%
>>"%DFTWORK_TEST_CAPTURE%" echo args=%ARGS%
>>"%DFTWORK_TEST_CAPTURE%" echo prompt=
type "%QUERY%" >>"%DFTWORK_TEST_CAPTURE%"
echo {"type":"text","text":"mock-response"}
echo {"type":"result","session_id":"session-next","exit_code":0,"text":"mock-response","tokens":{"input":1,"output":2,"total":3,"cache_read":0,"cache_write":0},"duration_ms":12}
"#,
    )
    .expect("写入 Hermes mock");
    path
}

#[cfg(windows)]
fn write_mock_hermes_scenario(dir: &Path, scenario: &str) -> PathBuf {
    let body = match scenario {
        "fragments" => {
            r#"echo {"type":"text","text":"\u7b2c\u4e00\u6bb5"}
echo {"type":"text","text":" + \u7b2c\u4e8c\u6bb5"}
echo {"type":"result","session_id":"session-next","exit_code":0,"tokens":{"input":1,"output":2,"total":3},"duration_ms":12}
"#
        }
        "process-error" => {
            r#"echo {"type":"result","session_id":"session-next","exit_code":0,"text":"false success"}
exit /b 9
"#
        }
        "bad-stream" => {
            r#"echo not-json
echo {"type":"result","session_id":"session-next","exit_code":0,"text":"false success"}
"#
        }
        "timeout" => "ping -n 21 127.0.0.1 >nul\r\n",
        _ => panic!("未知 mock 场景：{scenario}"),
    };
    let path = dir.join(format!("mock {scenario}.cmd"));
    std::fs::write(&path, format!("@echo off\r\n{body}")).expect("写入场景 Hermes mock");
    path
}

#[cfg(not(windows))]
fn write_mock_hermes_scenario(dir: &Path, scenario: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let body = match scenario {
        "fragments" => {
            r#"printf '%s\n' '{"type":"text","text":"第一段"}'
printf '%s\n' '{"type":"text","text":" + 第二段"}'
printf '%s\n' '{"type":"result","session_id":"session-next","exit_code":0,"tokens":{"input":1,"output":2,"total":3},"duration_ms":12}'
"#
        }
        "process-error" => {
            r#"printf '%s\n' '{"type":"result","session_id":"session-next","exit_code":0,"text":"false success"}'
exit 9
"#
        }
        "bad-stream" => {
            r#"printf '%s\n' 'not-json'
printf '%s\n' '{"type":"result","session_id":"session-next","exit_code":0,"text":"false success"}'
"#
        }
        "timeout" => "sleep 20\n",
        _ => panic!("未知 mock 场景：{scenario}"),
    };
    let path = dir.join(format!("mock {scenario}"));
    std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("写入场景 Hermes mock");
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    path
}

fn run_mock_chat(root: &Path, mock: &Path, prompt: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("hermes-chat")
        .arg(root)
        .arg(prompt)
        .args([
            "--model",
            "test-model",
            "--provider",
            "test-provider",
            "--timeout",
            "5",
        ])
        .env("HERMES_BIN", mock)
        .output()
        .expect("运行 Hermes 场景")
}

#[cfg(not(windows))]
fn write_mock_hermes(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("mock hermes");
    std::fs::write(
        &path,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'Hermes Agent v0.0.0-test'
  exit 0
fi
query=
args="$*"
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--query-file" ]; then query="$2"; fi
  shift
done
{
  printf 'cwd=%s\n' "$PWD"
  printf 'args=%s\n' "$args"
  printf 'prompt=\n'
  cat "$query"
} >> "$DFTWORK_TEST_CAPTURE"
printf '%s\n' '{"type":"text","text":"mock-response"}'
printf '%s\n' '{"type":"result","session_id":"session-next","exit_code":0,"text":"mock-response","tokens":{"input":1,"output":2,"total":3,"cache_read":0,"cache_write":0},"duration_ms":12}'
"#,
    )
    .expect("写入 Hermes mock");
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    path
}

#[test]
fn application_errors_use_stdout_json_and_keep_their_code() {
    let temp = TestDir::new("error-envelope");
    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("scan")
        .arg(temp.path().join("missing"))
        .output()
        .expect("运行 scan");

    assert!(!output.status.success(), "{}", failure_details(&output));
    assert!(output.stderr.is_empty(), "{}", failure_details(&output));
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout 必须是 JSON 信封");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "NOT_FOUND");
}

#[test]
fn malformed_arguments_use_stdout_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .args(["compare", "only-one"])
        .output()
        .expect("运行非法 compare");

    assert!(!output.status.success(), "{}", failure_details(&output));
    assert!(output.stderr.is_empty(), "{}", failure_details(&output));
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("参数错误也必须输出 JSON 信封");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "VALIDATION");
}

#[test]
fn hermes_probe_uses_the_configured_binary_with_spaces() {
    let temp = TestDir::new("probe-path");
    let mock = write_mock_hermes(temp.path());
    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("hermes-probe")
        .env("HERMES_BIN", &mock)
        .output()
        .expect("探测 Hermes");

    assert!(output.status.success(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["found"], true, "{}", failure_details(&output));
    assert_eq!(envelope["data"]["bin"], mock.to_string_lossy().as_ref());
    assert_eq!(envelope["data"]["version"], "0.0.0-test");
}

#[test]
fn hermes_chat_uses_project_cwd_message_and_resume() {
    let temp = TestDir::new("chat-cwd");
    let root = temp.path().join("project root");
    std::fs::create_dir_all(&root).unwrap();
    let mock = write_mock_hermes(temp.path());
    let prompt = temp.path().join("提问.txt");
    std::fs::write(&prompt, "第一个问题\n第二行").unwrap();
    let capture = temp.path().join("capture.txt");

    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("hermes-chat")
        .arg(&root)
        .arg(&prompt)
        .args([
            "--model",
            "test-model",
            "--provider",
            "test-provider",
            "--resume",
            "session-before",
        ])
        .env("HERMES_BIN", &mock)
        .env("DFTWORK_TEST_CAPTURE", &capture)
        .output()
        .expect("运行 Hermes 对话");

    assert!(output.status.success(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["text"], "mock-response");
    assert_eq!(envelope["data"]["session_id"], "session-next");

    let captured = std::fs::read_to_string(capture).expect("读取 mock 调用记录");
    assert!(captured.contains(&format!("cwd={}", root.display())), "{captured}");
    assert!(captured.contains("第一个问题\n第二行"), "{captured}");
    assert!(captured.contains("--resume session-before"), "{captured}");
    assert!(captured.contains("--model test-model"), "{captured}");
    assert!(captured.contains("--provider test-provider"), "{captured}");
}

#[test]
fn hermes_probe_reports_a_missing_configured_binary() {
    let temp = TestDir::new("probe-missing");
    let missing = temp.path().join("missing hermes.cmd");
    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("hermes-probe")
        .env("HERMES_BIN", &missing)
        .output()
        .expect("探测缺失的 Hermes");

    assert!(output.status.success(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["found"], false);
    assert!(envelope["data"]["reason"]
        .as_str()
        .unwrap()
        .contains(&missing.to_string_lossy().as_ref()));
}

#[test]
fn hermes_probe_falls_back_to_a_command_script_on_path() {
    let temp = TestDir::new("path-fallback");
    let mock = write_mock_hermes(temp.path());
    let script = temp.path().join(if cfg!(windows) { "hermes.cmd" } else { "hermes" });
    std::fs::rename(mock, &script).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("hermes-probe")
        .env_remove("HERMES_BIN")
        .env("PATH", temp.path())
        .output()
        .expect("通过 PATH 探测 Hermes");

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["found"], true, "{}", failure_details(&output));
}

#[test]
fn hermes_chat_concatenates_text_fragments() {
    let temp = TestDir::new("text-fragments");
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let prompt = temp.path().join("message.txt");
    std::fs::write(&prompt, "测试").unwrap();
    let mock = write_mock_hermes_scenario(temp.path(), "fragments");
    let output = run_mock_chat(&root, &mock, &prompt);

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["text"], "第一段 + 第二段", "{}", failure_details(&output));
    assert_eq!(envelope["data"]["exit_code"], 0);
    assert_eq!(envelope["data"]["tokens"]["total"], 3);
}

#[test]
fn hermes_chat_rejects_a_success_result_when_the_process_fails() {
    let temp = TestDir::new("process-error");
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let prompt = temp.path().join("message.txt");
    std::fs::write(&prompt, "测试").unwrap();
    let mock = write_mock_hermes_scenario(temp.path(), "process-error");
    let output = run_mock_chat(&root, &mock, &prompt);

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["exit_code"], 9, "{}", failure_details(&output));
}

#[test]
fn hermes_chat_rejects_malformed_jsonl() {
    let temp = TestDir::new("bad-stream");
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let prompt = temp.path().join("message.txt");
    std::fs::write(&prompt, "测试").unwrap();
    let mock = write_mock_hermes_scenario(temp.path(), "bad-stream");
    let output = run_mock_chat(&root, &mock, &prompt);

    assert!(!output.status.success(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "EXTERNAL");
}

#[test]
fn hermes_chat_kills_a_timed_out_process() {
    let temp = TestDir::new("timeout");
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let prompt = temp.path().join("message.txt");
    std::fs::write(&prompt, "测试").unwrap();
    let mock = write_mock_hermes_scenario(temp.path(), "timeout");
    let started = std::time::Instant::now();
    let output = run_mock_chat(&root, &mock, &prompt);

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["timed_out"], true, "{}", failure_details(&output));
    assert_ne!(envelope["data"]["exit_code"], 0);
    assert!(started.elapsed() < std::time::Duration::from_secs(15));
}

#[test]
fn conflict_errors_keep_their_code_in_stdout_json() {
    let temp = TestDir::new("conflict-envelope");
    let approved_root = temp.path().join("approved");
    let other_root = temp.path().join("other");
    std::fs::create_dir_all(&approved_root).unwrap();
    std::fs::create_dir_all(&other_root).unwrap();
    let items = vec![vasp_domain::BuildItem {
        rel_path: "task/INCAR".into(),
        content: "ENCUT = 520\n".into(),
        action: vasp_domain::WriteAction::Create,
        reason: "CLI contract regression".into(),
        previous_content: None,
    }];
    let plan_hash = vasp_domain::plan_hash::compute(&approved_root.to_string_lossy(), &items);
    let mut plan = vasp_domain::BuildPlan {
        root: approved_root.to_string_lossy().into_owned(),
        items,
        plan_hash,
        created_at: "2026-11-03T00:00:00+00:00".into(),
    };
    plan.root = other_root.to_string_lossy().into_owned();
    let plan_path = temp.path().join("plan.json");
    std::fs::write(&plan_path, serde_json::to_vec(&plan).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("apply")
        .arg(plan_path)
        .output()
        .expect("运行 apply");

    assert!(!output.status.success(), "{}", failure_details(&output));
    assert!(output.stderr.is_empty(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["error"]["code"], "CONFLICT");
    assert!(!other_root.join("task/INCAR").exists());
}
