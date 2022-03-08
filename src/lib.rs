#[macro_use]
extern crate log;
extern crate libc;
extern crate wait_timeout;

use anyhow::Result;
use std::io::Write;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::Duration;

pub use branches::Branch;
use docker::{Container, RunResult};

mod branches;
mod docker;
mod github;
pub mod routes;

pub use github::init_client as init_github_client;

pub struct Playpen;

impl Playpen {
    pub fn new() -> Playpen {
        Playpen
    }

    async fn exec(
        branch: Branch,
        cmd: &str,
        args: Vec<String>,
        input: String,
    ) -> Result<RunResult> {
        let container = Container::new(cmd, &args, &[], branch.image()).await?;
        container.run(input.as_bytes(), Duration::new(10, 0)).await
    }

    fn parse_output(raw: &[u8]) -> (String, String) {
        let mut split = raw.splitn(2, |b| *b == b'\xff');
        let compiler = String::from_utf8_lossy(split.next().unwrap_or(&[])).into_owned();
        let output = String::from_utf8_lossy(split.next().unwrap_or(&[])).into_owned();

        (compiler, output)
    }

    pub async fn evaluate(branch: Branch, code: String) -> Result<(RunResult, String, String)> {
        let result = Self::exec(branch, "/usr/local/bin/evaluate.sh", vec![], code).await?;
        let (compiler, output) = Self::parse_output(result.stdout());
        Ok((result, compiler, output))
    }

    pub async fn compile(
        branch: Branch,
        code: String,
        emit: CompileOutput,
    ) -> Result<(RunResult, String, String)> {
        let args = emit.as_opts().iter().map(|x| String::from(*x)).collect();
        let result = Self::exec(branch, "/usr/local/bin/compile.sh", args, code).await?;
        debug!("{:?}", result.result);
        debug!("{}", String::from_utf8_lossy(result.stdout()));
        debug!("{}", String::from_utf8_lossy(result.stderr()));
        let (compiler, output) = Self::parse_output(result.stdout());
        Ok((result, compiler, output))
    }
}

impl Default for Playpen {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CompileOutput {
    Asm,
    Llvm,
}

impl CompileOutput {
    pub fn as_opts(&self) -> &'static [&'static str] {
        match *self {
            CompileOutput::Asm => &["--pass=asm"],
            CompileOutput::Llvm => &["--pass=ir"],
        }
    }
}

impl FromStr for CompileOutput {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "asm" => Ok(CompileOutput::Asm),
            "llvm-ir" => Ok(CompileOutput::Llvm),
            _ => Err(format!("unknown output format {}", s)),
        }
    }
}

/// Highlights compiled asm or llvm ir output according to the given output format
pub fn highlight(output_format: CompileOutput, output: &str) -> String {
    let lexer = match output_format {
        CompileOutput::Asm => "gas",
        CompileOutput::Llvm => "llvm",
    };

    let mut child = Command::new("pygmentize")
        .arg("-l")
        .arg(lexer)
        .arg("-f")
        .arg("html")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}
