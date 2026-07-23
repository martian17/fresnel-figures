use std::io::Write;

use anyhow::Result;
use sim_runner::{fresnel_binary, fresnel_dir, run_telemetry};

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
    let fresnel = fresnel_dir();
    let binary = fresnel_binary()?;
    let cpus = cpu_order()?;
    eprintln!("cpu order: {cpus:?}");

    let mut csv = std::fs::File::create("core-scaling-vs-quisp.csv")?;
    writeln!(csv, "cores,cpu_list,events_per_second")?;

    let args = vec![format!("--arm1={ARM1_CM}"), format!("--arm2={ARM2_CM}")];
    for n in 1..=cpus.len() {
        let cpu_list = cpus[..n]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        match run_telemetry(&fresnel, &binary, &args, Some(&cpu_list), MEASURE_WINDOW_S) {
            Ok(mut rate) => {
                // hard code but will do for now
                // one packet at SPD equals 3 packets total
                rate *= 3.0;
                eprintln!("{n} cores ({cpu_list}): {rate:.1} events/s");
                writeln!(csv, "{n},\"{cpu_list}\",{rate}")?;
                csv.flush()?;
            }
            Err(e) => eprintln!("{n} cores ({cpu_list}): FAILED: {e:#}"),
        }
    }
    Ok(())
}
