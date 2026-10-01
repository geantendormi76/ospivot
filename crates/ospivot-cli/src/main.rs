use anyhow::Result;
use std::path::Path;
use ospivot_core::PipelineRunner;

fn main() -> Result<()> {
    let config_path = Path::new("configs/transition.toml");
    let blackboard_path = Path::new("artifacts/state/blackboard.json");

    let mut runner = PipelineRunner::new(config_path, blackboard_path)?;
    runner.run_all()?;

    Ok(())
}
