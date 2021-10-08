use std::process::ExitStatus;

use tokio::{
    io::{self, AsyncBufReadExt, AsyncRead, BufReader},
    process::Child,
    sync::{mpsc, oneshot},
    time::Instant,
};

pub async fn manage_process(
    mut process: Child,
    stdout_channel: mpsc::UnboundedSender<(String, Instant)>,
    stderr_channel: mpsc::UnboundedSender<(String, Instant)>,
    exit_channel: oneshot::Sender<io::Result<ExitStatus>>,
    kill_signal: oneshot::Receiver<()>,
) {
    let stdout = process.stdout.take().unwrap();
    let stderr = process.stderr.take().unwrap();

    // continously read from stdout/stderr in background
    let stdout_handle = tokio::spawn(read_to_end(stdout, stdout_channel));
    let stderr_handle = tokio::spawn(read_to_end(stderr, stderr_channel));

    // wait for either process to finish or receival of terminate command
    tokio::select! {
        _ = process.wait() => { }
        _ = kill_signal => {
            if let Err(kill_error) = process.kill().await {
                println!{"unexpected error when killing process, pid: {:?}, err: {}", process.id(), kill_error};
            }
        }
    }

    // wait for process to finish, send exit status / error on exit channel
    if let Err(join_error) = stdout_handle.await {
        println! {"unexpected error when joining stdout, pid: {:?}, err: {}", process.id(), join_error};
    }
    if let Err(join_error) = stderr_handle.await {
        println! {"unexpected error when joining stderr, pid: {:?}, err: {}", process.id(), join_error};
    }
    if let Err(send_error) = exit_channel.send(process.wait().await) {
        println! {"unexpected closed channel when sending exit result, pid: {:?}", process.id()};
        todo!()
    }
}

async fn read_to_end<A: AsyncRead + std::marker::Unpin>(
    stream: A,
    lines_sender: mpsc::UnboundedSender<(String, Instant)>,
) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut buf = String::new();
        match reader.read_line(&mut buf).await {
            Ok(n) if n == 0 => break,
            Ok(_) => {}
            Err(io_error) => match io_error.kind() {
                io::ErrorKind::InvalidData => buf.push_str("###INVALID UTF8###"),
                _ => println!("unexpected io error when reading from stream: {}", io_error),
            },
        }
        if let Err(send_error) = lines_sender.send((buf, Instant::now())) {
            println!(
                "unexpected error when sending stream output line : {}",
                send_error
            )
        }
    }
}

// actually receives one more line with timestamp after <until>
pub async fn receive_lines_until(
    lines_receiver: &mut mpsc::UnboundedReceiver<(String, Instant)>,
    until: &Instant,
) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        match lines_receiver.try_recv() {
            Ok((line, timestamp)) => {
                lines.push(line);
                if timestamp > *until {
                    break;
                }
            }
            Err(_err) => break,
        };
    }
    lines
}

pub async fn receive_all_lines(
    lines_receiver: &mut mpsc::UnboundedReceiver<(String, Instant)>,
) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        if let Some((line, _timestamp)) = lines_receiver.recv().await {
            lines.push(line);
        } else {
            break;
        }
    }
    lines
}
