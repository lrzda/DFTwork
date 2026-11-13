mod cli;
mod commands;
mod support;

use clap::{error::ErrorKind, Parser};
use cli::{Cli, Commands};

fn emit_error(code: &str, message: &str) {
    let payload = serde_json::json!({
        "ok": false,
        "error": { "code": code, "message": message }
    });
    match serde_json::to_string(&payload) {
        Ok(s) => println!("{s}"),
        Err(_) => println!("{{\"ok\":false,\"error\":{{\"code\":\"FAILED\",\"message\":\"错误序列化失败\"}}}}"),
    }
}

fn main() {
    let args = match Cli::try_parse() {
        Ok(args) => args,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            let _ = e.print();
            return;
        }
        Err(e) => {
            emit_error("VALIDATION", &e.to_string());
            std::process::exit(2);
        }
    };
    vasp_infrastructure::logging::init(args.verbose);

    let result = match args.command {
        Commands::Scan { root, max_depth } => commands::scan(&root, max_depth),
        Commands::Convergence { task } => commands::convergence(&task),
        Commands::Scene { file, no_pbc } => commands::scene(&file, no_pbc),
        Commands::Magnetization { task } => commands::magnetization(&task),
        Commands::Scf { task } => commands::scf(&task),
        Commands::Forces { task } => commands::forces(&task),
        Commands::Vibrations { task } => commands::vibrations(&task),
        Commands::Trajectory { task } => commands::trajectory(&task),
        Commands::Compare { tasks } => commands::compare(&tasks),
        Commands::Build { source, target, set, root, file } => {
            commands::build(&source, &target, &set, root.as_deref(), &file)
        }
        Commands::Apply { plan } => commands::apply(&plan),
        Commands::Rollback { receipt } => commands::rollback(&receipt),
        Commands::ProjectState { root } => commands::project_state(&root),
        Commands::DiscardPlan { root } => commands::discard_plan(&root),
        Commands::Recover { root } => commands::recover(&root),
        Commands::PruneBackups { root } => commands::prune_backups(&root),
        Commands::Analyze { task, model, provider, hermes_bin, max_turns, timeout } => {
            commands::analyze(
                &task,
                model.as_deref(),
                provider.as_deref(),
                hermes_bin.as_deref(),
                max_turns,
                timeout,
            )
        }
        Commands::HermesChat {
            root,
            prompt_file,
            model,
            provider,
            hermes_bin,
            resume,
            max_turns,
            timeout,
        } => commands::hermes_chat(
            &root,
            &prompt_file,
            model.as_deref(),
            provider.as_deref(),
            hermes_bin.as_deref(),
            resume.as_deref(),
            max_turns,
            timeout,
        ),
        Commands::HermesProbe { hermes_bin } => commands::hermes_probe(hermes_bin.as_deref()),
        Commands::Version => commands::version(),
    };

    if let Err(e) = result {
        let (code, message) = match e.downcast_ref::<vasp_application::AppError>() {
            Some(app) => (app.code(), app.message()),
            None => ("FAILED", e.to_string()),
        };
        emit_error(code, &message);
        std::process::exit(1);
    }
}
