#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("codexbar-linuxd: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--version") {
        println!(
            "{} {}",
            codexbar_linuxd::DAEMON_NAME,
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--check") {
        codexbar_linuxd::app::App::check_startup()?;
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--print-snapshot") {
        let app = codexbar_linuxd::app::App::from_env()?;
        println!("{}", app.get_snapshot_json()?);
        return Ok(());
    }

    codexbar_linuxd::dbus::serve_until_shutdown().await?;
    Ok(())
}

fn print_help() {
    println!(
        "{} {}\n\nUsage: {} [--check] [--version] [--print-snapshot]\n\nOptions:\n  --check           Validate daemon startup and version metadata, then exit.\n  --version         Print daemon version and exit.\n  --print-snapshot  Print the current daemon snapshot JSON and exit.\n  -h, --help        Show this help.\n",
        codexbar_linuxd::DAEMON_NAME,
        env!("CARGO_PKG_VERSION"),
        codexbar_linuxd::DAEMON_NAME,
    );
}
