use amux::app;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        // No subcommand — launch TUI
        return app::run();
    }

    // Serve subcommand
    if args[1] == "serve" {
        let port = args.iter().position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080);
        let token = args.iter().position(|a| a == "--token")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default();

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(amux::server::run_server(port, token))?;
        return Ok(());
    }

    // Headless subcommands: run, list, status
    if let Some(result) = amux::headless::try_headless(&args) {
        let code = result?;
        std::process::exit(code);
    }

    // Unknown subcommand — fall through to TUI
    app::run()
}