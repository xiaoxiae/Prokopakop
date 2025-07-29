use crate::GameController;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_position() {
    let mut controller = GameController::new();

    // This should just not crash hihi
    for position in [
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq -",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    ] {
        controller.new_game(Option::from(position));
    }
}

#[test]
fn test_perft_positions() {
    let mut controller = GameController::new();
    let mut failures: Vec<_> = Vec::new();
    let mut total = 0;

    // Yoinked from https://www.chessprogramming.org/Perft_Results
    let test_positions = [
        // Position 1: Starting position
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            vec![(1, 20), (2, 400), (3, 8902), (4, 197281), (5, 4865609)],
        ),
        // Position 2: Kiwipete
        (
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -",
            vec![(1, 48), (2, 2039), (3, 97862), (4, 4085603)],
        ),
        // Position 3: Position with en passant and castling
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -", vec![
            (1, 14),
            (2, 191),
            (3, 2812),
            (4, 43238),
        ]),
        // Position 4: Complex position with promotions
        (
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq -",
            vec![(1, 6), (2, 264), (3, 9467), (4, 422333)],
        ),
        // Position 5: Another complex position
        (
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            vec![(1, 44), (2, 1486), (3, 62379), (4, 2103487)],
        ),
        // Position 6: Balanced middle game position
        (
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            vec![(1, 46), (2, 2079), (3, 89890), (4, 3894594)],
        ),
    ];

    for (position_fen, depth_counts) in test_positions.iter() {
        println!("Testing position: {}", position_fen);
        controller.new_game(Some(position_fen));

        for &(depth, expected_count) in depth_counts {
            let start_time = Instant::now();
            let moves = controller.get_valid_moves(depth);
            let elapsed = start_time.elapsed();

            let total_nodes: usize = moves.iter().map(|(_, count)| count).sum();

            println!(
                "  Depth {}: {} nodes (expected: {}) - {:?}",
                depth, total_nodes, expected_count, elapsed
            );

            if total_nodes != expected_count {
                failures.push(format!(
                    "Position '{}' at depth {}: got {} nodes, expected {}",
                    position_fen, depth, total_nodes, expected_count
                ));
            }

            total += 1;
        }
        println!();
    }

    // Panic at the end with all failure information
    if !failures.is_empty() {
        let failure_summary = failures.join("\n  ");
        panic!(
            "Perft test failed with {}/{} error(s):\n  {}",
            failures.len(),
            total,
            failure_summary
        );
    }
}

#[derive(Debug)]
pub enum ProcessError {
    Io(std::io::Error),
    Timeout,
    ProcessTerminated,
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Io(e) => write!(f, "IO error: {}", e),
            ProcessError::Timeout => write!(f, "Operation timed out"),
            ProcessError::ProcessTerminated => write!(f, "Process terminated unexpectedly"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<std::io::Error> for ProcessError {
    fn from(error: std::io::Error) -> Self {
        ProcessError::Io(error)
    }
}

pub struct InteractiveProcess {
    child: Child,
    stdin: std::process::ChildStdin,
    line_receiver: mpsc::Receiver<Result<String, std::io::Error>>,
    _reader_thread: thread::JoinHandle<()>,
}

impl InteractiveProcess {
    pub fn new(command: &str, args: &[&str]) -> Result<Self, ProcessError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ProcessError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to open stdin",
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            ProcessError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to open stdout",
            ))
        })?;

        let (line_sender, line_receiver) = mpsc::channel();

        // Spawn a thread to read lines from stdout
        let reader_thread = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if line_sender.send(line).is_err() {
                    break; // Receiver dropped
                }
            }
        });

        Ok(InteractiveProcess {
            child,
            stdin,
            line_receiver,
            _reader_thread: reader_thread,
        })
    }

    pub fn write(&mut self, input: &str) -> Result<(), ProcessError> {
        self.stdin.write_all(input.as_bytes())?;
        if !input.ends_with('\n') {
            self.stdin.write_all(b"\n")?;
        }
        self.stdin.flush()?;
        Ok(())
    }

    pub fn read_line(&mut self, timeout: Duration) -> Result<String, ProcessError> {
        match self.line_receiver.recv_timeout(timeout) {
            Ok(Ok(line)) => Ok(line),
            Ok(Err(e)) => Err(ProcessError::Io(e)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProcessError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProcessError::ProcessTerminated),
        }
    }

    pub fn read_until<F>(
        &mut self,
        condition: F,
        timeout: Duration,
    ) -> Result<Vec<String>, ProcessError>
    where
        F: Fn(&str) -> bool,
    {
        let mut lines = Vec::new();
        let start_time = Instant::now();

        loop {
            let remaining_time = timeout.saturating_sub(start_time.elapsed());
            if remaining_time.is_zero() {
                return Err(ProcessError::Timeout);
            }

            let line = self.read_line(remaining_time)?;
            let trimmed = line.trim();
            lines.push(line.to_string());

            if condition(trimmed) {
                break;
            }
        }

        Ok(lines)
    }

    pub fn read_until_contains(
        &mut self,
        target: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, ProcessError> {
        self.read_until(|line| line.contains(target), timeout)
    }

    pub fn read_until_equals(
        &mut self,
        target: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, ProcessError> {
        self.read_until(|line| line == target, timeout)
    }

    pub fn write_and_read_until<F>(
        &mut self,
        input: &str,
        condition: F,
        timeout: Duration,
    ) -> Result<Vec<String>, ProcessError>
    where
        F: Fn(&str) -> bool,
    {
        self.write(input)?;
        self.read_until(condition, timeout)
    }

    pub fn write_and_read_until_contains(
        &mut self,
        input: &str,
        target: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, ProcessError> {
        self.write(input)?;
        self.read_until_contains(target, timeout)
    }

    pub fn write_and_read_until_equals(
        &mut self,
        input: &str,
        target: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, ProcessError> {
        self.write(input)?;
        self.read_until_equals(target, timeout)
    }

    pub fn terminate(mut self, timeout: Duration) -> Result<(), ProcessError> {
        // Close stdin to signal the process to exit
        drop(self.stdin);

        let start_time = Instant::now();
        loop {
            match self.child.try_wait()? {
                Some(_exit_status) => return Ok(()),
                None => {
                    if start_time.elapsed() >= timeout {
                        // Force kill the process
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return Err(ProcessError::Timeout);
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    pub fn is_alive(&mut self) -> Result<bool, ProcessError> {
        match self.child.try_wait()? {
            Some(_) => Ok(false),
            None => Ok(true),
        }
    }
}

#[test]
fn test_stockfish() -> Result<(), ProcessError> {
    let mut process = InteractiveProcess::new("bin/stockfish/src/stockfish", &[])?;

    let command_timeout = Duration::from_secs(5);

    process.write_and_read_until_equals("uci", "uciok", command_timeout)?;
    process.write_and_read_until_contains("go perft 1", "Nodes", command_timeout)?;
    process.write("quit")?;
    process.terminate(command_timeout)?;

    Ok(())
}

#[test]
fn test_timeout_failure() {
    let mut process = InteractiveProcess::new("bin/stockfish/src/stockfish", &[]).unwrap();

    let result = process.write_and_read_until_equals(
        "uci",
        "nonexistent_response",
        Duration::from_millis(100),
    );

    match result {
        Err(ProcessError::Timeout) => println!("Test correctly timed out"),
        _ => panic!("Expected timeout error"),
    }

    let _ = process.terminate(Duration::from_secs(1));
}

#[test]
fn test_start_and_quit() -> Result<(), ProcessError> {
    let mut process = InteractiveProcess::new("target/debug/Prokopakop", &[])?;

    let command_timeout = Duration::from_secs(1);

    process.write_and_read_until_equals("uci", "uciok", command_timeout)?;
    process.write("quit")?;
    process.terminate(command_timeout)?;

    Ok(())
}

#[test]
fn test_perft() -> Result<(), ProcessError> {
    let mut process = InteractiveProcess::new("target/debug/Prokopakop", &[])?;

    let command_timeout = Duration::from_secs(1);

    process.write_and_read_until_equals("uci", "uciok", command_timeout)?;
    process.write_and_read_until_contains("go perft 1", "Nodes", command_timeout)?;
    process.write("quit")?;
    process.terminate(command_timeout)?;

    Ok(())
}
