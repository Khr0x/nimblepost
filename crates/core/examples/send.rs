use nimblepost_core::{CancellationToken, ExecutionContext, execute, load_request, prepare};
use std::{collections::BTreeMap, error::Error};

/// Development example, not the future product CLI.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=4).contains(&args.len()) {
        return Err("Uso: send <colección> <request.yml> [environment|-] [baseUrl]".into());
    }
    let environment = args
        .get(2)
        .filter(|name| name.as_str() != "-")
        .map(String::as_str);
    let loaded = load_request(&args[0], &args[1], environment).await?;
    let context = ExecutionContext {
        overrides: args
            .get(3)
            .map(|url| BTreeMap::from([("baseUrl".into(), url.clone())]))
            .unwrap_or_default(),
        ..Default::default()
    };
    let prepared = prepare(&loaded, &context)?;
    let token = CancellationToken::new();
    let response = tokio::select! {
        result = execute(prepared, &token) => result?,
        result = tokio::signal::ctrl_c() => {
            result?;
            token.cancel();
            return Err(nimblepost_core::Error::Cancelled.into());
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": response.status,
            "headers": response.headers.iter().map(|header| serde_json::json!({
                "name": header.name,
                "value": String::from_utf8_lossy(&header.value)
            })).collect::<Vec<_>>(),
            "body": String::from_utf8_lossy(&response.body)
        }))?
    );
    Ok(())
}
