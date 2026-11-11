use anyhow::Result;
use std::os::unix::prelude::ExitStatusExt;
use std::process::{ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time;

const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum ChildResult {
    ExitCode(i32),
    #[allow(dead_code)] // we don't wanna let go of that signal
    Signal(i32),
    TimedOut,
    OutputLimitExceeded,
}

impl From<ExitStatus> for ChildResult {
    fn from(es: ExitStatus) -> Self {
        es.code()
            .map(ChildResult::ExitCode)
            .or_else(|| es.signal().map(ChildResult::Signal))
            .unwrap() // it should either be an exitcode or a signal
    }
}

#[derive(Debug)]
pub struct RunResult {
    pub(crate) result: ChildResult,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

impl RunResult {
    fn collected(result: ChildResult, output: CapturedOutput) -> Self {
        Self {
            result: if output.exceeded {
                ChildResult::OutputLimitExceeded
            } else {
                result
            },
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }

    pub(crate) fn stderr_text(&self) -> String {
        let mut text = String::from_utf8_lossy(self.stderr()).into_owned();
        if matches!(self.result, ChildResult::OutputLimitExceeded) {
            text.push_str(
                "\nSandbox output exceeded the 8 MiB limit (stdout and stderr combined).\n",
            );
        }
        text
    }

    pub(crate) fn stdout(&self) -> &[u8] {
        self.stdout.as_slice()
    }

    pub(crate) fn stderr(&self) -> &[u8] {
        self.stderr.as_slice()
    }

    pub(crate) fn success(&self) -> bool {
        matches!(self.result, ChildResult::ExitCode(0))
    }
}

pub struct Container {
    id: String,
}

impl Container {
    pub async fn new(
        cmd: &str,
        args: &[String],
        env: &[(String, String)],
        name: &str,
    ) -> Result<Container> {
        let out = run(Command::new("docker")
            .arg("create")
            .arg("--cap-drop=ALL")
            .arg("--memory=1024m")
            .arg("--net=none")
            .arg("--pids-limit=20")
            .arg("--security-opt=no-new-privileges")
            .arg("--interactive")
            .args(
                env.iter()
                    .map(|(k, v)| format!("--env={}={}", k, v))
                    .collect::<Vec<_>>(),
            )
            .arg(name)
            .arg(cmd)
            .stderr(Stdio::inherit())
            .args(args))
        .await?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        Ok(Container {
            id: stdout.trim().to_string(),
        })
    }

    pub async fn run(&self, input: &[u8], timeout: Duration) -> Result<RunResult> {
        let mut cmd = Command::new("docker");
        cmd.arg("start")
            .arg("--attach")
            .arg("--interactive")
            .arg(&self.id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true); // ensure we kill the process when leavin this func if it timed out
        debug!("attaching with {:?}", cmd);
        let start = Instant::now();
        let mut cmd = cmd.spawn()?;
        cmd.stdin.take().unwrap().write_all(input).await?;
        debug!("input written, now waiting");

        let mut output = CapturedOutput::default();
        let mut child_stdout = cmd.stdout.take().unwrap();
        let mut child_stderr = cmd.stderr.take().unwrap();
        let result = time::timeout(timeout, async {
            output
                .read(&mut child_stdout, &mut child_stderr, true)
                .await?;
            if output.exceeded {
                Ok::<_, std::io::Error>(None)
            } else {
                Ok(Some(cmd.wait().await?))
            }
        })
        .await;

        let result = match result {
            Ok(Ok(Some(status))) => status.into(),
            Ok(Ok(None)) => ChildResult::OutputLimitExceeded,
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => ChildResult::TimedOut,
        };
        if matches!(
            result,
            ChildResult::TimedOut | ChildResult::OutputLimitExceeded
        ) {
            tokio::try_join!(
                self.stop(),
                async { Ok::<_, anyhow::Error>(cmd.wait().await?) },
                async {
                    output
                        .read(&mut child_stdout, &mut child_stderr, false)
                        .await?;
                    Ok::<_, anyhow::Error>(())
                },
            )?;
        }
        debug!("timing: {:?}", start.elapsed());
        Ok(RunResult::collected(result, output))
    }

    async fn stop(&self) -> Result<()> {
        run(Command::new("docker")
            .arg("stop")
            .arg("-t")
            .arg("0")
            .arg(&self.id))
        .await?;
        Ok(())
    }
}

#[derive(Default)]
struct CapturedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exceeded: bool,
}

impl CapturedOutput {
    async fn read(
        &mut self,
        stdout: &mut (impl AsyncRead + Unpin),
        stderr: &mut (impl AsyncRead + Unpin),
        stop_at_limit: bool,
    ) -> std::io::Result<()> {
        let mut stdout_chunk = [0; 8192];
        let mut stderr_chunk = [0; 8192];
        let mut stdout_open = true;
        let mut stderr_open = true;
        while stdout_open || stderr_open {
            let (is_stdout, bytes) = tokio::select! {
                bytes = stdout.read(&mut stdout_chunk), if stdout_open => (true, bytes?),
                bytes = stderr.read(&mut stderr_chunk), if stderr_open => (false, bytes?),
            };
            if bytes == 0 {
                if is_stdout {
                    stdout_open = false;
                } else {
                    stderr_open = false;
                }
                continue;
            }
            let remaining = OUTPUT_LIMIT - self.stdout.len() - self.stderr.len();
            let retained = bytes.min(remaining);
            let (buffer, chunk) = if is_stdout {
                (&mut self.stdout, &stdout_chunk)
            } else {
                (&mut self.stderr, &stderr_chunk)
            };
            buffer.extend_from_slice(&chunk[..retained]);
            self.exceeded |= bytes > remaining;
            if self.exceeded && stop_at_limit {
                return Ok(());
            }
        }
        Ok(())
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        let rt = tokio::runtime::Handle::current();
        let id = self.id.clone();
        rt.spawn(
            Command::new("docker")
                .arg("rm")
                .arg("--force")
                .arg(&id)
                .output(),
        );
    }
}

async fn run(cmd: &mut Command) -> Result<Output> {
    debug!("spawning: {:?}", cmd);
    let start = Instant::now();
    let out = cmd.output().await?;
    debug!("done in: {:?}", start.elapsed());
    debug!("output: {:?}", out);
    if !out.status.success() {
        anyhow::bail!("process failed: {:?}\n{:?}", cmd, out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn output_boundaries() -> Result<()> {
        for is_stdout in [true, false] {
            for size in [0, OUTPUT_LIMIT - 1, OUTPUT_LIMIT, OUTPUT_LIMIT + 1] {
                let bytes = vec![b'x'; size];
                let (mut stdout, mut stderr) = if is_stdout {
                    (bytes.as_slice(), &b""[..])
                } else {
                    (&b""[..], bytes.as_slice())
                };
                let mut output = CapturedOutput::default();
                output.read(&mut stdout, &mut stderr, true).await?;
                assert_eq!(output.exceeded, size > OUTPUT_LIMIT);
                let (captured, other) = if is_stdout {
                    (&output.stdout, &output.stderr)
                } else {
                    (&output.stderr, &output.stdout)
                };
                assert_eq!(captured.len(), size.min(OUTPUT_LIMIT));
                assert_eq!(captured, &bytes[..captured.len()]);
                assert!(other.is_empty());
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn streams_share_output_budget() -> Result<()> {
        for extra in [0, 1] {
            let stdout = vec![b'o'; OUTPUT_LIMIT / 2];
            let stderr = vec![b'e'; OUTPUT_LIMIT / 2 + extra];
            let mut output = CapturedOutput::default();
            output
                .read(&mut stdout.as_slice(), &mut stderr.as_slice(), true)
                .await?;
            assert_eq!(output.stdout.len() + output.stderr.len(), OUTPUT_LIMIT);
            assert_eq!(output.exceeded, extra > 0);
            assert_eq!(output.stdout, &stdout[..output.stdout.len()]);
            assert_eq!(output.stderr, &stderr[..output.stderr.len()]);
        }
        Ok(())
    }

    #[tokio::test]
    async fn timeout_cleanup_keeps_remaining_budget() -> Result<()> {
        for is_stdout in [true, false] {
            for extra in [0, 1] {
                let (mut out_writer, mut reader) = tokio::io::duplex(OUTPUT_LIMIT);
                let (mut err_writer, mut stderr) = tokio::io::duplex(OUTPUT_LIMIT);
                let writer = if is_stdout {
                    err_writer.shutdown().await?;
                    &mut out_writer
                } else {
                    out_writer.shutdown().await?;
                    &mut err_writer
                };
                let prefix = vec![b'p'; OUTPUT_LIMIT / 2];
                writer.write_all(&prefix).await?;
                let mut output = CapturedOutput::default();
                tokio::select! {
                    biased;
                    result = tokio::task::unconstrained(
                        output.read(&mut reader, &mut stderr, true),
                    ) => {
                        panic!("reader completed before cancellation: {result:?}");
                    }
                    _ = std::future::ready(()) => {}
                }
                let captured = if is_stdout {
                    &output.stdout
                } else {
                    &output.stderr
                };
                assert_eq!(captured, &prefix);
                let suffix = vec![b's'; OUTPUT_LIMIT / 2 + extra];
                writer.write_all(&suffix).await?;
                writer.shutdown().await?;
                output.read(&mut reader, &mut stderr, false).await?;
                let (captured, other) = if is_stdout {
                    (&output.stdout, &output.stderr)
                } else {
                    (&output.stderr, &output.stdout)
                };
                assert_eq!(captured.len(), OUTPUT_LIMIT);
                assert_eq!(&captured[..prefix.len()], prefix);
                assert_eq!(&captured[prefix.len()..], &suffix[..OUTPUT_LIMIT / 2]);
                assert!(other.is_empty());
                let result = RunResult::collected(ChildResult::TimedOut, output);
                if extra > 0 {
                    assert!(matches!(result.result, ChildResult::OutputLimitExceeded));
                } else {
                    assert!(matches!(result.result, ChildResult::TimedOut));
                }
            }
        }
        Ok(())
    }

    async fn run_script(script: &str, timeout: Duration) -> Result<RunResult> {
        let container =
            Container::new("sh", &["-c".into(), script.into()], &[], "ponylang-playpen").await?;
        let result = time::timeout(Duration::from_secs(30), container.run(b"", timeout)).await;
        let state = Command::new("docker")
            .args(["inspect", "--format", "{{.State.Running}}", &container.id])
            .output()
            .await;
        let removed = Command::new("docker")
            .args(["rm", "--force", &container.id])
            .output()
            .await?;
        assert!(removed.status.success(), "{removed:?}");
        let state = state?;
        assert!(state.status.success(), "{state:?}");
        let result = result
            .map_err(|error| anyhow::anyhow!("sandbox deadline: {script}: {error}"))?
            .map_err(|error| anyhow::anyhow!("sandbox command: {script}: {error}"))?;
        assert_eq!(state.stdout, b"false\n");
        Ok(result)
    }

    #[tokio::test]
    #[ignore]
    async fn container_output_boundaries() -> Result<()> {
        for redirect in ["", ">&2"] {
            for exit in [0, 1] {
                for size in [
                    0,
                    OUTPUT_LIMIT - 1,
                    OUTPUT_LIMIT,
                    OUTPUT_LIMIT + 1,
                    16 * OUTPUT_LIMIT,
                ] {
                    let result = run_script(
                        &format!("head -c {size} /dev/zero {redirect}; exit {exit}"),
                        Duration::from_secs(10),
                    )
                    .await?;
                    let (captured, other) = if redirect.is_empty() {
                        (result.stdout(), result.stderr())
                    } else {
                        (result.stderr(), result.stdout())
                    };
                    assert_eq!(captured.len(), size.min(OUTPUT_LIMIT));
                    assert!(captured.iter().all(|byte| *byte == 0));
                    assert!(other.is_empty());
                    if size > OUTPUT_LIMIT {
                        assert!(matches!(result.result, ChildResult::OutputLimitExceeded));
                        assert!(!result.success());
                    } else {
                        assert!(
                            matches!(result.result, ChildResult::ExitCode(code) if code == exit)
                        );
                        assert_eq!(result.success(), exit == 0);
                    }
                }
            }
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn container_mixed_and_continuous_output() -> Result<()> {
        for extra in [0, 1] {
            let half = OUTPUT_LIMIT / 2;
            let err_size = half + extra;
            let result = run_script(
                &format!("head -c {half} /dev/zero; head -c {err_size} /dev/zero >&2"),
                Duration::from_secs(10),
            )
            .await?;
            assert_eq!(result.stdout.len() + result.stderr.len(), OUTPUT_LIMIT);
            assert!(
                result
                    .stdout
                    .iter()
                    .chain(&result.stderr)
                    .all(|byte| *byte == 0)
            );
            assert_eq!(result.success(), extra == 0);
            if extra > 0 {
                assert!(matches!(result.result, ChildResult::OutputLimitExceeded));
            }
        }
        for script in ["yes x", "yes x >&2", "yes x & yes x >&2; wait"] {
            let result = run_script(script, Duration::from_secs(10)).await?;
            assert!(matches!(result.result, ChildResult::OutputLimitExceeded));
            assert_eq!(result.stdout.len() + result.stderr.len(), OUTPUT_LIMIT);
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn container_timeout_preserves_output() -> Result<()> {
        let result = run_script(
            "printf out; printf err >&2; sleep 30",
            Duration::from_secs(2),
        )
        .await?;
        assert!(matches!(result.result, ChildResult::TimedOut));
        assert_eq!(result.stdout(), b"out");
        assert_eq!(result.stderr(), b"err");
        assert!(!result.success());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn stopping_exited_container_succeeds() -> Result<()> {
        let container = Container::new(
            "sh",
            &["-c".into(), "exit 0".into()],
            &[],
            "ponylang-playpen",
        )
        .await?;
        let result = container.run(b"", Duration::from_secs(10)).await?;
        let state = Command::new("docker")
            .args(["inspect", "--format", "{{.State.Running}}", &container.id])
            .output()
            .await?;
        let stopped = container.stop().await;
        let removed = Command::new("docker")
            .args(["rm", "--force", &container.id])
            .output()
            .await?;
        assert!(removed.status.success());
        assert!(result.success());
        assert!(state.status.success());
        assert_eq!(state.stdout, b"false\n");
        stopped?;
        Ok(())
    }
}
