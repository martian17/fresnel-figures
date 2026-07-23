use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use arrow::array::UInt64Array;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

pub struct RunStats {
    pub packets: u64,
    pub first_ps: u64,
    pub last_ps: u64,
    pub wall_secs: f64,
}

impl RunStats {
    pub fn sim_secs(&self) -> f64 {
        (self.last_ps - self.first_ps) as f64 / 1.0e12
    }
    pub fn packets_per_sim_sec(&self) -> f64 {
        self.packets as f64 / self.sim_secs()
    }
    pub fn packets_per_wall_sec(&self) -> f64 {
        self.packets as f64 / self.wall_secs
    }
}

// Kills the child on drop so an error path never leaks a running simulation.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn parquet_files(data_dir: &Path) -> Result<HashSet<PathBuf>> {
    let mut files = HashSet::new();
    for entry in std::fs::read_dir(data_dir)
        .with_context(|| format!("reading {}", data_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "parquet") {
            files.insert(path);
        }
    }
    Ok(files)
}

fn scan_file(path: &Path) -> Result<(u64, u64, u64)> {
    let file = File::open(path)?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("opening {} (was the simulation shut down gracefully so the parquet footer got written?)", path.display()))?
        .build()?;
    let mut count = 0u64;
    let mut first = u64::MAX;
    let mut last = 0u64;
    for batch in reader {
        let batch = batch?;
        let tags = batch
            .column_by_name("time_tag")
            .context("no time_tag column")?
            .as_any()
            .downcast_ref::<UInt64Array>()
            .context("time_tag is not u64")?;
        count += tags.len() as u64;
        for &t in tags.values().iter() {
            first = first.min(t);
            last = last.max(t);
        }
    }
    Ok((count, first, last))
}

// Runs the fresnel binary (optionally pinned with `taskset -c <cpu_list>`)
// for `run_duration` of wall time, interrupts it, then counts the time tags
// in the parquet file(s) the run produced and deletes them.
//
// Only files that did not exist before the run are read and removed, so
// pre-existing data in fresnel/data is never touched.
pub fn run_once(
    fresnel_dir: &Path,
    binary: &Path,
    extra_args: &[String],
    cpu_list: Option<&str>,
    run_duration: Duration,
) -> Result<RunStats> {
    let data_dir = fresnel_dir.join("data");
    std::fs::create_dir_all(&data_dir)?;
    let before = parquet_files(&data_dir)?;

    let mut cmd = match cpu_list {
        Some(list) => {
            let mut c = Command::new("taskset");
            c.arg("-c").arg(list).arg(binary);
            c
        }
        None => Command::new(binary),
    };
    let mut child = ChildGuard(
        cmd.args(extra_args)
            .current_dir(fresnel_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawning fresnel")?,
    );

    let start = Instant::now();
    std::thread::sleep(run_duration);
    let wall_secs = start.elapsed().as_secs_f64();

    // SIGINT so the binary can flush and close the parquet writer
    unsafe { libc::kill(child.0.id() as libc::pid_t, libc::SIGINT) };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.0.try_wait()?.is_some() {
            break;
        }
        if Instant::now() > deadline {
            bail!("fresnel did not exit within 10s of SIGINT; the parquet file would be truncated");
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let new_files: Vec<PathBuf> = parquet_files(&data_dir)?
        .into_iter()
        .filter(|p| !before.contains(p))
        .collect();
    if new_files.is_empty() {
        bail!("run produced no parquet file in {}", data_dir.display());
    }

    let mut packets = 0u64;
    let mut first_ps = u64::MAX;
    let mut last_ps = 0u64;
    let mut scan_err = None;
    for path in &new_files {
        match scan_file(path) {
            Ok((c, f, l)) => {
                packets += c;
                first_ps = first_ps.min(f);
                last_ps = last_ps.max(l);
            }
            Err(e) => scan_err = Some(e),
        }
    }
    for path in &new_files {
        let _ = std::fs::remove_file(path);
    }
    if let Some(e) = scan_err {
        return Err(e);
    }
    if packets == 0 {
        bail!("run produced a parquet file with zero time tags");
    }

    Ok(RunStats {
        packets,
        first_ps,
        last_ps,
        wall_secs,
    })
}

pub struct TelemetrySample {
    pub elapsed_s: u64,
    pub total_packets: u64,
}

// Parses one line of the fresnel throughput monitor, e.g.
// `[   3s] SPD1:    123456 pkt/s | ... | total: 246912 pkts, 0.000123 sim-s`
pub fn parse_telemetry_line(line: &str) -> Option<TelemetrySample> {
    let elapsed_s = line
        .strip_prefix('[')?
        .split_once("s]")?
        .0
        .trim()
        .parse()
        .ok()?;
    let total_packets = line
        .split_once("total: ")?
        .1
        .split_once(" pkts")?
        .0
        .parse()
        .ok()?;
    Some(TelemetrySample {
        elapsed_s,
        total_packets,
    })
}

// Runs the fresnel binary (optionally pinned with `taskset -c <cpu_list>`)
// and measures throughput from its stdout telemetry instead of the parquet
// output: packets counted between the first monitor line and the first line
// at least `window_secs` later, divided by that (wall-clock) span. Skipping
// the pre-first-line span excludes startup transients.
//
// Any parquet files the run creates are deleted; pre-existing ones are kept.
pub fn run_telemetry(
    fresnel_dir: &Path,
    binary: &Path,
    extra_args: &[String],
    cpu_list: Option<&str>,
    window_secs: u64,
) -> Result<f64> {
    use std::io::BufRead;

    let data_dir = fresnel_dir.join("data");
    std::fs::create_dir_all(&data_dir)?;
    let before = parquet_files(&data_dir)?;

    let mut cmd = match cpu_list {
        Some(list) => {
            let mut c = Command::new("taskset");
            c.arg("-c").arg(list).arg(binary);
            c
        }
        None => Command::new(binary),
    };
    let mut child = ChildGuard(
        cmd.args(extra_args)
            .current_dir(fresnel_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("spawning fresnel")?,
    );

    let stdout = child.0.stdout.take().context("no stdout pipe")?;
    let mut first: Option<TelemetrySample> = None;
    let mut rate = None;
    for line in std::io::BufReader::new(stdout).lines() {
        let Some(sample) = parse_telemetry_line(&line?) else {
            continue;
        };
        match &first {
            None => first = Some(sample),
            Some(f) if sample.elapsed_s >= f.elapsed_s + window_secs => {
                rate = Some(
                    (sample.total_packets - f.total_packets) as f64
                        / (sample.elapsed_s - f.elapsed_s) as f64,
                );
                break;
            }
            Some(_) => {}
        }
    }
    // ChildGuard kills the process; no graceful shutdown is needed since the
    // measurement never touches the parquet output
    drop(child);

    for path in parquet_files(&data_dir)? {
        if !before.contains(&path) {
            let _ = std::fs::remove_file(&path);
        }
    }

    rate.context("fresnel exited before emitting enough telemetry lines")
}

// The prebuilt simulator binary; building it is the fresnel side's job,
// so a sweep never triggers (or waits on) a compile.
pub fn fresnel_binary() -> Result<PathBuf> {
    let path = fresnel_dir().join("target/release/fresnel");
    if !path.is_file() {
        bail!(
            "{} not found; build fresnel with cargo build --release first",
            path.display()
        );
    }
    Ok(path)
}

pub fn fresnel_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    Path::new(&home).join("fresnel")
}
