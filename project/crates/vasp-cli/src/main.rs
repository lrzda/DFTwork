mod cli;
mod commands;
mod support;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let args = Cli::parse();
    vasp_infrastructure::logging::init(args.verbose);

    let result = match args.command {
        Commands::Scan { root, max_depth } => commands::scan(&root, max_depth),
        Commands::Convergence { task } => commands::convergence(&task),
        Commands::Scene { file, no_pbc } => commands::scene(&file, no_pbc),
        Commands::HermesProbe => commands::hermes_probe(),
        Commands::Version => commands::version(),
    };

    if let Err(e) = result {
        // 应用层错误自带错误码（NOT_FOUND / VALIDATION / …），不要一律降级成 FAILED
        let (code, message) = match e.downcast_ref::<vasp_application::AppError>() {
            Some(app) => (app.code(), app.message()),
            None => ("FAILED", e.to_string()),
        };
        let payload = serde_json::json!({
            "ok": false,
            "error": { "code": code, "message": message }
        });
        match serde_json::to_string(&payload) {
            Ok(s) => eprintln!("{s}"),
            Err(_) => eprintln!("{{\"ok\":false,\"error\":{{\"code\":\"FAILED\"}}}}"),
        }
        std::process::exit(1);
    }
}
