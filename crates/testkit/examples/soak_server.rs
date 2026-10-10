//! The fault-injecting test server on its own, for the soak test (`tools/soak.sh`).
//!
//! Serves one file of known content per size, each on its own port, and prints
//! one line per file, then `ready`:
//!
//! ```text
//! file <size> <sha256> http://127.0.0.1:<port>/file.bin
//! ready
//! ```
//!
//! It runs until its standard input closes (Ctrl-D, or the script that started
//! it exits, even if that script is killed), then prints
//! `stats <requests> <faults>` and stops. It drains its request log as it goes,
//! so memory stays flat through a 24 h soak.
//!
//! ```text
//! cargo run --release -p fuselane-testkit --example soak_server -- --sizes 1048576 --fault drops
//! ```

use std::io::Read;
use std::process::ExitCode;
use std::time::Duration;

use fuselane_testkit::{Content, Fault, RangeServer, Rule};

const USAGE: &str = "\
Usage: soak_server --sizes BYTES[,BYTES...] [--fault MODE] [--every N] [--seed N]

  --sizes   file sizes in bytes; one file (and port) per size
  --fault   none (default), drops, resets, stalls, busy, slow, lies, mixed
  --every   misbehave on every Nth request to each file (default 20, at least 2)
  --seed    changes the file contents and fault details (default 1)

Fault modes:
  drops   the connection closes partway through the answer
  resets  the connection closes before any answer
  stalls  the answer stops partway and the connection stays open, silent
  busy    503 or 429 with Retry-After: 1
  slow    every answer is sent at 1 MiB/s (all requests, not every Nth)
  lies    wrong ranges: extra bytes, an earlier start, a shorter range, no Content-Range
  mixed   drops, resets, busy and lies in turn

Stops when standard input closes.";

/// Per-answer speed in `slow` mode; Fuselane opens several streams, so a file
/// still arrives at a few MiB/s.
const SLOW_BYTES_PER_SEC: u64 = 1 << 20;
/// How often the log is drained and the next fault is scheduled.
const HOUSEKEEPING: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    None,
    Drops,
    Resets,
    Stalls,
    Busy,
    Slow,
    Lies,
    Mixed,
}

impl Mode {
    fn parse(s: &str) -> Option<Mode> {
        Some(match s {
            "none" => Mode::None,
            "drops" => Mode::Drops,
            "resets" => Mode::Resets,
            "stalls" => Mode::Stalls,
            "busy" => Mode::Busy,
            "slow" => Mode::Slow,
            "lies" => Mode::Lies,
            "mixed" => Mode::Mixed,
            _ => return None,
        })
    }

    /// Modes that fault one request in every N (rather than none, or all).
    fn periodic(self) -> bool {
        !matches!(self, Mode::None | Mode::Slow)
    }
}

#[derive(Debug)]
struct Options {
    sizes: Vec<u64>,
    mode: Mode,
    every: u32,
    seed: u64,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut opts = Options {
        sizes: Vec::new(),
        mode: Mode::None,
        every: 20,
        seed: 1,
    };
    while let Some(flag) = args.next() {
        if flag == "-h" || flag == "--help" {
            return Ok(None);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{flag} needs a value."))?;
        match flag.as_str() {
            "--sizes" => {
                for part in value.split(',') {
                    let n = part.trim().parse::<u64>().map_err(|_| {
                        format!(
                            "\"{part}\" isn't a size in bytes. Example: --sizes 1048576,8388608"
                        )
                    })?;
                    opts.sizes.push(n);
                }
            }
            "--fault" => {
                opts.mode = Mode::parse(&value)
                    .ok_or_else(|| format!("there's no fault mode called \"{value}\"."))?;
            }
            "--every" => {
                opts.every = value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n >= 2)
                    .ok_or_else(|| {
                        format!("--every needs a whole number of 2 or more, not \"{value}\".")
                    })?;
            }
            "--seed" => {
                opts.seed = value
                    .parse::<u64>()
                    .map_err(|_| format!("--seed needs a whole number, not \"{value}\"."))?;
            }
            _ => return Err(format!("there's no option called \"{flag}\".")),
        }
    }
    if opts.sizes.is_empty() {
        return Err("give at least one file size with --sizes.".into());
    }
    Ok(Some(opts))
}

/// xorshift64: varied fault details that a seed reproduces.
#[derive(Debug)]
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }
}

/// The next fault for `mode`; `turn` counts faults so modes rotate their variants.
fn next_fault(mode: Mode, turn: u64, rng: &mut Rng) -> Fault {
    const KB: u64 = 1024;
    match mode {
        Mode::Drops => Fault::ShortBody(rng.below(256 * KB)),
        Mode::Resets => Fault::Reset,
        Mode::Stalls => Fault::StallAt(rng.below(256 * KB)),
        Mode::Busy => Fault::Status(
            if turn.is_multiple_of(2) { 503 } else { 429 },
            Some("1".into()),
        ),
        Mode::Lies => match turn % 4 {
            0 => Fault::Overrun(1 + rng.below(9_000)),
            1 => Fault::WrongStart(1 + rng.below(9_000)),
            2 => Fault::CapRange(1 + rng.below(64 * KB)),
            _ => Fault::NoContentRange,
        },
        Mode::Mixed => {
            let sub = [Mode::Drops, Mode::Resets, Mode::Busy, Mode::Lies];
            let pick = sub[(turn % sub.len() as u64) as usize];
            next_fault(pick, turn / sub.len() as u64, rng)
        }
        Mode::None | Mode::Slow => Fault::Throttle(SLOW_BYTES_PER_SEC),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::main]
async fn main() -> ExitCode {
    let opts = match parse_args(std::env::args().skip(1)) {
        Ok(Some(o)) => o,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(msg) => {
            eprintln!("soak_server: {msg} Run it with --help to see the options.");
            return ExitCode::from(2);
        }
    };

    let mut servers = Vec::with_capacity(opts.sizes.len());
    for (i, &size) in opts.sizes.iter().enumerate() {
        let content = Content::new(size, opts.seed.wrapping_add(i as u64));
        let server = match RangeServer::start(content).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "soak_server: couldn't open a port on 127.0.0.1 ({e}). Check that loopback networking works."
                );
                return ExitCode::FAILURE;
            }
        };
        if opts.mode == Mode::Slow {
            server.add_rule(Rule::always(Fault::Throttle(SLOW_BYTES_PER_SEC)));
        }
        // Hashing a large file takes a while; keep the runtime free meanwhile.
        let sha = match tokio::task::spawn_blocking(move || content.sha256()).await {
            Ok(sha) => sha,
            Err(e) => {
                eprintln!("soak_server: couldn't hash the test file: {e}");
                return ExitCode::FAILURE;
            }
        };
        println!(
            "file {size} {} http://{}{}",
            hex(&sha),
            server.addr(),
            server.path()
        );
        servers.push(server);
    }
    println!("ready");

    // Standard input closing is the signal to stop.
    let (tx, mut stop) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 256];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = tx.send(());
    });

    let mut rng = Rng(opts.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut turn: u64 = 0;
    let (mut requests, mut faults) = (0u64, 0u64);
    let mut tick = tokio::time::interval(HOUSEKEEPING);
    loop {
        tokio::select! {
            _ = &mut stop => break,
            _ = tick.tick() => {
                for server in &servers {
                    let log = server.take_requests();
                    requests += log.len() as u64;
                    faults += log.iter().filter(|r| r.fault.is_some()).count() as u64;
                    if opts.mode.periodic() && server.pending_rules() == 0 {
                        server.add_rule(Rule {
                            skip: opts.every - 1,
                            times: 1,
                            fault: next_fault(opts.mode, turn, &mut rng),
                        });
                        turn += 1;
                    }
                }
            }
        }
    }
    for server in &servers {
        let log = server.take_requests();
        requests += log.len() as u64;
        faults += log.iter().filter(|r| r.fault.is_some()).count() as u64;
    }
    println!("stats {requests} {faults}");
    ExitCode::SUCCESS
}
