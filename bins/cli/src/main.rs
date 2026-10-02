use base64::Engine;
use clap::{Parser, Subcommand};
use nimblepost_core::{
    CancellationToken, Document, Error, ExecutionContext, LoadedRequest, execute, load_request,
    prepare,
};
use std::{collections::BTreeMap, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "nimblepost",
    version,
    about = "Execute local OpenCollection HTTP requests"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Execute one YAML request using the shared Rust core.
    Run {
        request: PathBuf,
        #[arg(short, long)]
        collection: Option<PathBuf>,
        #[arg(short, long, requires = "collection")]
        env: Option<String>,
        #[arg(long = "var", value_name = "NAME=VALUE", value_parser = parse_variable)]
        variables: Vec<(String, String)>,
        /// Supply a declared secret from one explicitly named process variable.
        #[arg(long = "secret-env", value_name = "NAME=ENV_VAR", value_parser = parse_variable)]
        secret_environment: Vec<(String, String)>,
    },
}

fn parse_variable(text: &str) -> std::result::Result<(String, String), String> {
    let (name, value) = text.split_once('=').ok_or("Use NAME=VALUE")?;
    if name.is_empty() {
        return Err("Variable name cannot be empty".into());
    }
    Ok((name.into(), value.into()))
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Cancelled) => {
            eprintln!("Petición cancelada");
            ExitCode::from(130)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> nimblepost_core::Result<()> {
    let Command::Run {
        request,
        collection,
        env,
        variables,
        secret_environment,
    } = args.command;
    let mut overrides = BTreeMap::new();
    for (name, value) in variables {
        if overrides.insert(name, value).is_some() {
            return Err(Error::Config {
                path: request,
                field: "--var".into(),
                message: "Variable duplicada",
            });
        }
    }
    let mut secrets = BTreeMap::new();
    for (name, variable) in secret_environment {
        let value = std::env::var(variable).map_err(|_| Error::Config {
            path: request.clone(),
            field: "--secret-env".into(),
            message: "La variable de entorno indicada no contiene un string disponible",
        })?;
        if secrets.insert(name, value).is_some() {
            return Err(Error::Config {
                path: request.clone(),
                field: "--secret-env".into(),
                message: "Secreto duplicado",
            });
        }
    }
    let context = ExecutionContext { overrides, secrets };
    let cancel = CancellationToken::new();
    let result = tokio::select! {
        result = async {
            let loaded = if let Some(root) = collection { load_request(root, &request, env.as_deref()).await? }
                else { LoadedRequest::standalone(Document::read(&request).await?)? };
            execute(prepare(&loaded, &context)?, &cancel).await
        } => result,
        _ = tokio::signal::ctrl_c() => { cancel.cancel(); Err(Error::Cancelled) }
    }?;
    let body_text = std::str::from_utf8(&result.body).ok();
    let output = serde_json::json!({
        "status": result.status,
        "headers": result.headers.iter().map(|header| serde_json::json!({
            "name": header.name, "value": String::from_utf8_lossy(&header.value),
            "valueBase64": base64::engine::general_purpose::STANDARD.encode(&header.value)
        })).collect::<Vec<_>>(),
        "body": body_text.map(str::to_owned).unwrap_or_else(|| base64::engine::general_purpose::STANDARD.encode(&result.body)),
        "bodyEncoding": if body_text.is_some() { "utf8" } else { "base64" },
        "bodyBytes": result.body.len()
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("JSON values serialize")
    );
    Ok(())
}
