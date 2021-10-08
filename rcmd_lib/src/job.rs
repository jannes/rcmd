pub struct CommandSpec {
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct JobOutput {
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
}

impl JobOutput {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            stdout_lines: Vec::new(),
            stderr_lines: Vec::new(),
        }
    }

    pub fn append(&mut self, mut stdout_lines: Vec<String>, mut stderr_lines: Vec<String>) {
        self.stdout_lines.append(&mut stdout_lines);
        self.stderr_lines.append(&mut stderr_lines);
    }

    pub fn stdout(&self) -> String {
        self.stdout_lines.join("")
    }

    pub fn stderr(&self) -> String {
        self.stderr_lines.join("")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Completed { exit_code: i32 },
    Terminated,
    Error { msg: String },
}
