use tokio::{io::AsyncRead, process::ChildStderr};

async fn consume_outstream(outstream: ChildStderr) -> String {
    todo!()
    // let mut buffer = [0; 512];
    // let mut text = Vec::new();
    // 'l: loop {
    //     tokio::select! {
    //         result = outstream.read(&mut buffer) => {
    //             let bytes_read = result?;
    //             if bytes_read == 0 {
    //                 break 'l;
    //             }
    //             text.extend_from_slice(&buffer[0..bytes_read]);
    //         }
    //         _ = sleep(Duration::from_millis(5)) => {
    //             break 'l;
    //         }
    //     };
    // }
    // let text = String::from_utf8(text).unwrap_or_else(|_| "NON-UTF8".to_string());
    // text
}
