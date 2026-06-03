use amux::app;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // Check for --web flag (enables embedded HTTP server in TUI mode)
    let web_mode = args.iter().any(|a| a == "--web" || a == "-w");
    if args.len() < 2 || (args.len() == 2 && web_mode) {
        // No subcommand — launch TUI (optionally with web server)
        return app::run(web_mode);
    }

    // Serve subcommand
    if args[1] == "serve" {
        let port = args
            .iter()
            .position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080);
        let token = args
            .iter()
            .position(|a| a == "--token")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default();

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(amux::server::run_server(port, token))?;
        return Ok(());
    }
    // Doctor subcommand
    if args[1] == "doctor" {
        let results = amux::doctor::run_doctor();
        let mut failed = 0usize;
        for r in &results {
            let icon = if r.passed { "✓" } else { "✗" };
            println!("{} {}", icon, r.name);
            if !r.message.is_empty() {
                println!("  {}", r.message);
            }
            if let Some(ref hint) = r.fix_hint {
                println!("  Fix: {}", hint);
            }
            if !r.passed {
                failed += 1;
            }
        }
        if failed > 0 {
            println!();
            println!("{} check(s) failed.", failed);
            std::process::exit(1);
        }
        return Ok(());
    }

    // Headless subcommands: run, list, status
    if let Some(result) = amux::headless::try_headless(&args) {
        let code = result?;
        std::process::exit(code);
    }

    // Unknown subcommand — fall through to TUI
    app::run(web_mode)
}
