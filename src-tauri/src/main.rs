fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Hidden acceptance mode: --acceptance-scenario <name>
    if args.len() >= 3 && args[1] == "--acceptance-scenario" {
        let scenario_name = &args[2];
        let scenario = lifesub_lib::acceptance::AcceptanceScenario::from_arg(scenario_name);
        let Some(scenario) = scenario else {
            eprintln!("unknown acceptance scenario: {scenario_name}");
            eprintln!("valid scenarios: real-asr-heartbeat, cancel-real-asr, claim-and-abort, verify-recovery, packaged-smoke");
            std::process::exit(1);
        };

        // Determine report path and data directory
        let report_dir = std::env::var("LIFESUB_ACCEPTANCE_DIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
        let report_path = std::path::PathBuf::from(&report_dir)
            .join(format!("acceptance-{}.json", scenario.as_str()));

        let data_dir = std::env::var("LIFESUB_ACCEPTANCE_DATA_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."))
            });

        std::fs::create_dir_all(&report_dir).ok();
        std::fs::create_dir_all(&data_dir).ok();

        let mut ctx = lifesub_lib::acceptance::AcceptanceContext::new(
            scenario.clone(),
            report_path.clone(),
            data_dir,
        );

        let passed = lifesub_lib::acceptance::run_scenario(&mut ctx);

        if passed {
            println!("ACCEPTANCE PASSED: {}", scenario.as_str());
        } else {
            eprintln!("ACCEPTANCE FAILED: {}", scenario.as_str());
            std::process::exit(1);
        }
        return;
    }

    // Normal desktop launch
    lifesub_lib::run();
}

fn dirs_next() -> Option<std::path::PathBuf> {
    // Use a simple fallback for the data directory
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|home| {
            std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.goldenwave.lifesub")
        })
        .ok()
}