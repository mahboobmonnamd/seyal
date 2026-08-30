use std::{
    io::{BufRead, BufReader, Read},
    sync::mpsc,
    thread,
};

// A dedicated reader thread decouples the caller's wait from the pipe: a
// blocking `recv_timeout` on the channel always returns within the caller's
// chosen timeout even if the underlying `read_line` call on that thread is
// itself stuck, so a stalled or dead subprocess fails loudly instead of
// hanging the caller. Used both for the fresh Runtime worker's stdio (see
// `worker::RuntimeWorker`) and for the isolated `--cohort` subprocess (see
// `orchestrator::run_cohort_process`).
pub(crate) fn spawn_line_reader<R: Read + Send + 'static>(source: R) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(source);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if tx.send(line.trim_end().to_owned()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}
