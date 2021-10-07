use std::process::ExitStatus;

use tokio::{io::{self, AsyncBufReadExt, AsyncRead, BufReader}, process::Child, sync::{mpsc, oneshot}};

pub async fn manage_process(
    mut process: Child,
    stdout_channel: mpsc::Sender<String>,
    stderr_channel: mpsc::Sender<String>,
    exit_channel: mpsc::Sender<io::Result<ExitStatus>>,
    kill_channel: oneshot::Receiver<()>,
) {
    let stdout = process.stdout.take().unwrap();
    let stderr = process.stderr.take().unwrap();

    // continously read from stdout/stderr in background
    let stdout_handle = tokio::spawn(read_to_end(stdout, stdout_channel));
    let stderr_handle = tokio::spawn(read_to_end(stderr, stderr_channel));

    // either wait for proces to finish or receive a terminate command
    tokio::select! {
        _ = process.wait() => { }
        _ = kill_channel => {
            if let Err(kill_error) = process.kill().await {
                todo!()
            }
        }
    }

    // wait for process to finish, send exit status / error on exit channel
    if let Err(join_error) = stdout_handle.await {
        todo!()
    }
    if let Err(join_error) = stderr_handle.await {
        todo!()
    }
    if let Err(send_error) = exit_channel.send(process.wait().await).await {
        todo!()
    }
}

async fn read_to_end<A: AsyncRead + std::marker::Unpin>(
    stream: A,
    out_channel: mpsc::Sender<String>,
) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut buf = String::new();
        match reader.read_line(&mut buf).await {
            Ok(n) if n == 0 => break,
            Ok(_) => {}
            Err(io_error) => todo!(),
        }
        if let Err(send_error) = out_channel.send(buf).await {
            todo!()
        }
    }
}
