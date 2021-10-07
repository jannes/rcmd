use std::process::ExitStatus;

use tokio::{
    io::{self, AsyncBufReadExt, AsyncRead, BufReader},
    process::Child,
    sync::mpsc,
};

pub async fn manage_process(
    mut process: Child,
    stdout_channel: mpsc::Sender<String>,
    stderr_channel: mpsc::Sender<String>,
    exit_channel: mpsc::Sender<io::Result<ExitStatus>>,
    mut kill_channel: mpsc::Receiver<()>,
) {
    let stdout = process.stdout.take().unwrap();
    let stderr = process.stderr.take().unwrap();

    // continously read from stdout/stderr in background
    let _ = tokio::spawn(async { read_to_end(stdout, stdout_channel) });
    let _ = tokio::spawn(async {
        read_to_end(stderr, stderr_channel).await;
    });

    // either wait for proces to finish or receive a terminate command
    tokio::select! {
        _ = process.wait() => { }
        _ = kill_channel.recv() => {
            if let Err(e) = process.kill().await {
                todo!()
            }
            kill_channel.close()
        }
    }

    // wait for process to finish, send exit status / error on exit channel
    let res = exit_channel.send(process.wait().await).await;
    if let Err(send_error) = res {
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
            Err(_) => todo!(),
        }
        match out_channel.send(buf).await {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        }
    }
}
