use std::io::Write;
use std::env;
use std::path::{PathBuf};

use anyhow::Result;
use sim_runner::{run_telemetry};

// balanced arms (full HOM interference) as the representative workload
const ARM1_CM: u32 = 15;
const ARM2_CM: u32 = 15;
// packets are counted between the first telemetry line and the first one
// >= this many seconds later, so startup transients stay out of the rate
const MEASURE_WINDOW_S: u64 = 2;

// All CPU ids in plain numerical order, so SMT siblings are pinned right
// alongside their physical core as the sweep grows (0,1 share a core on
// this machine, then 2,3, ...).
fn cpu_order() -> Result<Vec<usize>> {
    let mut cpus = Vec::new();
    for entry in std::fs::read_dir("/sys/devices/system/cpu")? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some(id) = name
            .strip_prefix("cpu")
            .and_then(|s| s.parse::<usize>().ok())
        {
            cpus.push(id);
        }
    }
    cpus.sort_unstable();
    Ok(cpus)
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.is_empty() {
        panic!("You must provide the path");
    }
    let fresnel = PathBuf::from(args[1].clone());
    let binary = fresnel.join("target/release/fresnel");
    let cpus = cpu_order()?;
    eprintln!("cpu order: {cpus:?}");

    let mut csv = std::fs::File::create("core-scaling.csv")?;
    writeln!(csv, "cores,cpu_list,events_per_second")?;

    let args = vec![format!("--arm1={ARM1_CM}"), format!("--arm2={ARM2_CM}")];
    for n in 1..=cpus.len() {
        let cpu_list = cpus[..n]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        match run_telemetry(&fresnel, &binary, &args, Some(&cpu_list), MEASURE_WINDOW_S) {
            Ok(rate) => {
                // 4 packets arrive at the sensor. 2 packets enter the beam splitter, before that 2 packets
                eprintln!("{n} cores ({cpu_list}): {rate:.1} events/s");
                writeln!(csv, "{n},\"{cpu_list}\",{rate}")?;
                csv.flush()?;
            }
            Err(e) => eprintln!("{n} cores ({cpu_list}): FAILED: {e:#}"),
        }
    }
    Ok(())
}
