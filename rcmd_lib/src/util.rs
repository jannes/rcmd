use std::process::ExitStatus;

use tokio::{
    io::{self, AsyncBufReadExt, AsyncRead, BufReader},
    process::Child,
    sync::{mpsc, oneshot},
    time::Instant,
};
use tracing::{debug, error, info, instrument};

/// setup tasks to forward stdout/stderr to given channels
/// waits for process exiting or kill signal before sending exit status on given channel
#[instrument(skip(process, stdout_channel, stderr_channel, exit_channel, kill_signal))]
pub async fn manage_process(
    job_id: u64,
    mut process: Child,
    stdout_channel: mpsc::UnboundedSender<(String, Instant)>,
    stderr_channel: mpsc::UnboundedSender<(String, Instant)>,
    exit_channel: oneshot::Sender<io::Result<ExitStatus>>,
    kill_signal: oneshot::Receiver<()>,
) {
    info!("start managing process with pid: {:?}", process.id());
    let stdout = process.stdout.take().unwrap();
    let stderr = process.stderr.take().unwrap();

    // continously read from stdout/stderr in background
    let stdout_handle = tokio::spawn(read_to_end(stdout, stdout_channel));
    let stderr_handle = tokio::spawn(read_to_end(stderr, stderr_channel));

    // wait for either process to finish or receival of terminate command
    tokio::select! {
        _ = process.wait() => info!("process exited"),
        recv_res = kill_signal => {
            info!("received kill signal");
            if let Err(_recv_err) = recv_res {
                // this would happen when job pool was dropped
                debug!("kill channel receive error, sender dropped, pid: {:?}", process.id());
            }
            // kill process no matter if channel was closed or signal was sent
            if let Err(kill_error) = process.kill().await {
                error!("unexpected error when killing process, pid: {:?}, err: {}", process.id(), kill_error);
            }
        }
    }

    // wait for process to finish, send exit status / error on exit channel
    if let Err(join_error) = stdout_handle.await {
        error!(
            "unexpected error when joining stdout, pid: {:?}, err: {}",
            process.id(),
            join_error
        );
    }
    if let Err(join_error) = stderr_handle.await {
        error!(
            "unexpected error when joining stderr, pid: {:?}, err: {}",
            process.id(),
            join_error
        );
    }
    if let Err(_unsent) = exit_channel.send(process.wait().await) {
        // this would happen when job pool was dropped
        debug!(
            "closed channel when sending exit result, pid: {:?}",
            process.id()
        );
    }
}

/// get all lines from channel up to timestamp <until>
/// actually receives one more line with timestamp after <until>
pub async fn receive_lines_until(
    lines_receiver: &mut mpsc::UnboundedReceiver<(String, Instant)>,
    until: &Instant,
) -> Vec<String> {
    let mut lines = Vec::new();
    while let Ok((line, timestamp)) = lines_receiver.try_recv() {
        lines.push(line);
        if timestamp > *until {
            break;
        }
    }
    lines
}

/// get all lines from channel until all senders have dropped
pub async fn receive_all_lines(
    lines_receiver: &mut mpsc::UnboundedReceiver<(String, Instant)>,
) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some((line, _timestamp)) = lines_receiver.recv().await {
        lines.push(line);
    }
    lines
}

/// reads from stream and sends to channel line by line until EOF
/// when encountering invalid utf8 a marker is added to the line
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
                _ => error!("unexpected io error when reading from stream: {}", io_error),
            },
        }
        if let Err(send_error) = lines_sender.send((buf, Instant::now())) {
            error!(
                "unexpected error when sending stream output line : {}",
                send_error
            )
        }
    }
}
