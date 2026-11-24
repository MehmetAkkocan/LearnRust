//! Öğrenci kodu koşucusu: rustc ile derler, çalıştırır, çıktıyı kırpar.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;

const COMPILE_TIMEOUT: Duration = Duration::from_secs(30);
const RUN_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OUT: usize = 8 * 1024;

// Çalıştırıcının çıktıya eklediği notlar (arayüz İngilizceyken `i18n` çevirir).
pub const MSG_TRUNCATED: &str = "... [çıktı 8 KB sınırında kırpıldı]";
pub const MSG_COMPILE_TIMEOUT: &str = "derleme süresi doldu (30 sn)";
pub const MSG_RUN_TIMEOUT: &str = "çalışma süresi doldu (10 sn)";
pub const MSG_FLOOD: &str = "çıktı sınırı aşıldı, program durduruldu";

/// Eşzamanlı derleme/çalıştırma sınırı.
static SEM: Semaphore = Semaphore::const_new(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Passed,
    TestsFailed,
    CompileError,
    Timeout,
    Error,
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub outcome: RunOutcome,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

fn truncate(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    if s.len() <= MAX_OUT {
        return s.into_owned();
    }
    let mut end = MAX_OUT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n{}", &s[..end], MSG_TRUNCATED)
}

fn rustc_path() -> String {
    std::env::var("RUSTC_PATH").unwrap_or_else(|_| "rustc".to_string())
}

fn clean_env(cmd: &mut Command, toolchain: bool) {
    cmd.env_clear();
    // rustup proxy rustc için bu değişkenlere ihtiyaç duyar; öğrenci binary'sine verilmez.
    let extra: &[&str] = if toolchain {
        &["RUSTUP_HOME", "CARGO_HOME", "RUSTUP_TOOLCHAIN"]
    } else {
        &[]
    };
    for k in ["PATH", "HOME"].iter().chain(extra.iter()) {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }
}

struct TmpDir(PathBuf);
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn res(outcome: RunOutcome, stdout: String, stderr: String, start: Instant) -> RunResult {
    RunResult {
        outcome,
        stdout,
        stderr,
        duration_ms: start.elapsed().as_millis(),
    }
}

async fn run_inner(source: &str, test_mode: bool) -> RunResult {
    let start = Instant::now();
    let _permit = match SEM.acquire().await {
        Ok(p) => p,
        Err(e) => return res(RunOutcome::Error, String::new(), e.to_string(), start),
    };
    let dir = std::env::temp_dir().join(format!("rla-{:016x}", rand::random::<u64>()));
    let _guard = TmpDir(dir.clone());
    if let Err(e) = std::fs::create_dir(&dir) {
        return res(RunOutcome::Error, String::new(), format!("geçici dizin: {e}"), start);
    }
    let src = dir.join("main.rs");
    let bin: PathBuf = dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
    if let Err(e) = std::fs::write(&src, source) {
        return res(RunOutcome::Error, String::new(), format!("yazılamadı: {e}"), start);
    }

    // Derleme
    let mut c = Command::new(rustc_path());
    c.arg("--edition").arg("2021");
    if test_mode {
        c.arg("--test");
    }
    c.args(["-C", "opt-level=0", "--color", "never", "--crate-name", "student"])
        .arg(&src)
        .arg("-o")
        .arg(&bin)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    clean_env(&mut c, true);
    let out = match timeout(COMPILE_TIMEOUT, c.output()).await {
        Err(_) => {
            return res(
                RunOutcome::Timeout,
                String::new(),
                MSG_COMPILE_TIMEOUT.into(),
                start,
            )
        }
        Ok(Err(e)) => {
            return res(
                RunOutcome::Error,
                String::new(),
                format!("rustc başlatılamadı: {e}"),
                start,
            )
        }
        Ok(Ok(o)) => o,
    };
    if !out.status.success() {
        return res(
            RunOutcome::CompileError,
            truncate(&out.stdout),
            truncate(&out.stderr),
            start,
        );
    }

    run_binary(&bin, &dir, test_mode, start).await
}

/// Akıştan en fazla MAX_OUT bayt tutar; sınır aşılırsa `over` bildirilir ve okuma durur.
async fn read_capped<R: tokio::io::AsyncRead + Unpin>(
    mut rd: R,
    over: std::sync::Arc<tokio::sync::Notify>,
) -> (Vec<u8>, bool) {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match rd.read(&mut chunk).await {
            Ok(0) | Err(_) => return (buf, false),
            Ok(n) => {
                let room = MAX_OUT.saturating_sub(buf.len());
                buf.extend_from_slice(&chunk[..n.min(room)]);
                if n > room {
                    over.notify_one();
                    return (buf, true);
                }
            }
        }
    }
}

fn lossy_trunc(buf: &[u8], cut: bool) -> String {
    let mut s = String::from_utf8_lossy(buf).into_owned();
    if cut {
        s.push('\n');
        s.push_str(MSG_TRUNCATED);
    }
    s
}

/// Çocuk süreci (Unix'te tüm süreç grubunu) öldürür.
/// Drop'ta (istemci iptali dahil) süreç grubunu öldürür; `disarm` ile iptal edilir.
struct PgGuard(Option<u32>);
impl PgGuard {
    fn disarm(&mut self) {
        self.0 = None;
    }
}
impl Drop for PgGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.0 {
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("kill -9 -- -{pid}"))
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

async fn kill_tree(pid: Option<u32>, child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        let _ = Command::new("sh")
            .arg("-c")
            .arg(format!("kill -9 -- -{pid}"))
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.kill().await;
}

/// Test çıktısında "test result: ok. N passed" (N>0) var mı?
fn tests_really_passed(stdout: &str) -> bool {
    stdout.lines().any(|l| {
        l.strip_prefix("test result: ok. ")
            .and_then(|r| r.split_whitespace().next()?.parse::<u32>().ok())
            .is_some_and(|n| n > 0)
    })
}

async fn run_binary(bin: &Path, dir: &Path, test_mode: bool, start: Instant) -> RunResult {
    use std::sync::Arc;
    let mut r = Command::new(bin);
    if test_mode {
        r.arg("--test-threads=1");
    }
    r.current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    r.process_group(0);
    clean_env(&mut r, false);
    let mut child = match r.spawn() {
        Ok(c) => c,
        Err(e) => {
            return res(
                RunOutcome::Error,
                String::new(),
                format!("çalıştırılamadı: {e}"),
                start,
            )
        }
    };
    let pid = child.id();
    let mut guard = PgGuard(pid);
    let over = Arc::new(tokio::sync::Notify::new());
    let so = child.stdout.take().unwrap();
    let se = child.stderr.take().unwrap();
    let h_out = tokio::spawn(read_capped(so, over.clone()));
    let h_err = tokio::spawn(read_capped(se, over.clone()));

    enum End {
        Exit(std::io::Result<std::process::ExitStatus>),
        Timeout,
        Flood,
    }
    let end = tokio::select! {
        w = child.wait() => End::Exit(w),
        _ = tokio::time::sleep(RUN_TIMEOUT) => End::Timeout,
        _ = over.notified() => End::Flood,
    };
    // Normal çıkışta da arka planda kalan süreçleri öldür.
    kill_tree(pid, &mut child).await;
    guard.disarm();
    let grace = Duration::from_secs(2);
    let (ob, oc) = timeout(grace, h_out).await.ok().and_then(|j| j.ok()).unwrap_or_default();
    let (eb, ec) = timeout(grace, h_err).await.ok().and_then(|j| j.ok()).unwrap_or_default();
    let stdout = lossy_trunc(&ob, oc);
    let stderr = lossy_trunc(&eb, ec);
    match end {
        End::Timeout => res(
            RunOutcome::Timeout,
            stdout,
            format!("{stderr}\n{MSG_RUN_TIMEOUT}"),
            start,
        ),
        End::Flood => res(
            if test_mode { RunOutcome::TestsFailed } else { RunOutcome::Error },
            stdout,
            format!("{stderr}\n{MSG_FLOOD}"),
            start,
        ),
        End::Exit(Err(e)) => res(RunOutcome::Error, stdout, e.to_string(), start),
        End::Exit(Ok(st)) => {
            let outcome = if test_mode {
                if st.success() && tests_really_passed(&stdout) {
                    RunOutcome::Passed
                } else {
                    RunOutcome::TestsFailed
                }
            } else if st.success() {
                RunOutcome::Passed
            } else {
                RunOutcome::Error
            };
            res(outcome, stdout, stderr, start)
        }
    }
}

/// Öğrenci kodu + gizli testleri derleyip koşar.
pub async fn run_tests(student_code: &str, tests: &str) -> RunResult {
    let source = format!("{student_code}\n\n{tests}");
    run_inner(&source, true).await
}

/// Tam programı (fn main) derleyip çalıştırır; çıkış kodu 0 ise Passed.
pub async fn run_program(source: &str) -> RunResult {
    run_inner(source, false).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTS: &str = "#[cfg(test)] mod tests { use super::*; #[test] fn t() { assert_eq!(double(4), 8); } }";

    #[tokio::test]
    async fn gecen_test() {
        let r = run_tests("fn double(x: i32) -> i32 { x * 2 }", TESTS).await;
        assert_eq!(r.outcome, RunOutcome::Passed, "{}", r.stderr);
    }

    #[tokio::test]
    async fn basarisiz_test() {
        let r = run_tests("fn double(x: i32) -> i32 { x + 1 }", TESTS).await;
        assert_eq!(r.outcome, RunOutcome::TestsFailed, "{}", r.stderr);
    }

    #[tokio::test]
    async fn derleme_hatasi() {
        let r = run_tests("fn double(x: i32) -> i32 { x * }", TESTS).await;
        assert_eq!(r.outcome, RunOutcome::CompileError);
        assert!(r.stderr.contains("error"));
    }

    #[tokio::test]
    async fn sonsuz_dongu_timeout() {
        let r = run_tests(
            "fn double(_x: i32) -> i32 { loop { std::hint::spin_loop(); } }",
            TESTS,
        )
        .await;
        assert_eq!(r.outcome, RunOutcome::Timeout);
        assert!(r.duration_ms >= 10_000);
    }

    #[tokio::test]
    async fn program_calisir() {
        let r = run_program("fn main() { println!(\"selam\"); }").await;
        assert_eq!(r.outcome, RunOutcome::Passed);
        assert!(r.stdout.contains("selam"));
    }

    #[tokio::test]
    async fn cikti_bombasi_kirpilir() {
        let t = Instant::now();
        let r = run_program("fn main() { loop { println!(\"xxxxxxxxxxxxxxxxxxxxxxxx\"); } }").await;
        assert_eq!(r.outcome, RunOutcome::Error);
        assert!(r.stdout.len() < MAX_OUT + 200, "{}", r.stdout.len());
        assert!(r.stdout.contains("kırpıldı"));
        assert!(t.elapsed() < Duration::from_secs(10) + Duration::from_secs(30));
        assert!(r.duration_ms < 10_000);
    }

    #[tokio::test]
    async fn erken_exit_sifir_gecmez() {
        let r = run_tests(
            "fn double(_x: i32) -> i32 { std::process::exit(0) }",
            TESTS,
        )
        .await;
        assert_eq!(r.outcome, RunOutcome::TestsFailed);
    }
}
