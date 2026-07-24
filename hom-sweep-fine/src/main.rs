use std::io::Write;
use std::time::Duration;
use std::env;
use std::path::{PathBuf};

use anyhow::Result;
use sim_runner::{run_once};

// arm1 stays fixed while arm2 sweeps in 0.1 cm steps, so the path
// difference arm2 - arm1 covers -15.0..=+15.0 cm
const ARM1_CM: f64 = 15.0;
const ARM2_STEPS: std::ops::RangeInclusive<i64> = 0..=300;
const STEP_CM: f64 = 0.1;
const RUN_DURATION: Duration = Duration::from_millis(100);

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.is_empty() {
        panic!("You must provide the path");
    }
    let fresnel = PathBuf::from(args[1].clone());
    let binary = fresnel.join("target/release/fresnel");

    let mut csv = std::fs::File::create("hong-ou-mandel-fine.csv")?;
    writeln!(csv, "delta_cm,arm2_cm,packets_per_second")?;

    for step in ARM2_STEPS {
        let arm2_cm = step as f64 * STEP_CM;
        let delta_cm = arm2_cm - ARM1_CM;
        let args = vec![
            format!("--arm1={ARM1_CM}"),
            format!("--arm2={arm2_cm:.1}"),
        ];
        match run_once(&fresnel, &binary, &args, None, RUN_DURATION) {
            Ok(stats) => {
                let rate = stats.packets_per_sim_sec();
                eprintln!(
                    "delta {delta_cm:+.1} cm (arm2 {arm2_cm:.1} cm): {} packets over {:.6} sim-s -> {rate:.1} pkt/s",
                    stats.packets,
                    stats.sim_secs(),
                );
                writeln!(csv, "{delta_cm:.1},{arm2_cm:.1},{rate}")?;
                csv.flush()?;
            }
            Err(e) => eprintln!("delta {delta_cm:+.1} cm (arm2 {arm2_cm:.1} cm): FAILED: {e:#}"),
        }
    }
    Ok(())
}
