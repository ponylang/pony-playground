use anyhow::Result;
use std::os::unix::prelude::ExitStatusExt;
use std::process::{ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time;

#[derive(Debug)]
pub(crate) enum ChildResult {
    ExitCode(i32),
    Signal(i32),
    TimedOut,
}

impl From<ExitStatus> for ChildResult {
    fn from(es: ExitStatus) -> Self {
        es.code()
            .map(ChildResult::ExitCode)
            .or_else(|| es.signal().map(ChildResult::Signal))
            .unwrap() // it should either a exitcode or a signal
    }
}

#[derive(Debug)]
pub struct RunResult {
    pub(crate) result: ChildResult,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

impl RunResult {
    fn new(status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            result: status.into(),
            stdout,
            stderr,
        }
    }

    fn timed_out(stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            result: ChildResult::TimedOut,
            stdout,
            stderr,
        }
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
                &env.iter()
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

        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        let mut child_stdout = cmd.stdout.take().unwrap();
        let mut child_stderr = cmd.stderr.take().unwrap();

        match time::timeout(
            timeout,
            futures::future::join3(
                cmd.wait(),
                child_stdout.read_to_end(&mut stdout_buf),
                child_stderr.read_to_end(&mut stderr_buf),
            ),
        )
        .await
        {
            Err(_) => {
                // timeout
                debug!("timed out: {:?}", start.elapsed());
                // kill docker container
                run(Command::new("docker").arg("kill").arg(&self.id)).await?;
                // try waiting again
                let (_status, stderr, stdout) = futures::future::join3(
                    cmd.wait(),
                    child_stdout.read_to_end(&mut stdout_buf),
                    child_stderr.read_to_end(&mut stderr_buf),
                )
                .await;
                {
                    Ok(RunResult::timed_out(
                        stdout.map(|_| stdout_buf)?,
                        stderr.map(|_| stderr_buf)?,
                    ))
                }
            }
            Ok((status, stdout, stderr)) => {
                debug!("timing: {:?}", start.elapsed());
                Ok(RunResult::new(
                    status?,
                    stdout.map(|_| stdout_buf)?,
                    stderr.map(|_| stderr_buf)?,
                ))
            }
        }
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
