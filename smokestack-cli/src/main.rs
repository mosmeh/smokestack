mod component;
mod create;
mod list;
mod subscription;
mod tag;

use clap::{Parser, Subcommand};
use component::ComponentCommand;
use create::CreateArgs;
use http::{HeaderMap, HeaderValue};
use list::ListArgs;
use reqwest::{Response, Url};
use serde::{de::DeserializeOwned, Serialize};
use smokestack::{
    api::{ApiResponse, AuthRequest, AuthResponse, UpdateOperationRequest},
    model::{Claims, Operation, OperationState},
};
use std::{ffi::OsString, io::Write, path::Path, process::Stdio};
use subscription::SubscribeArgs;
use syntect::{
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};
use tag::TagCommand;

#[derive(Debug, Parser)]
#[clap(version)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    endpoint: Url,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new operation
    Create(CreateArgs),

    /// Show an operation
    Show { operation_id: u64 },

    /// List operations
    List(ListArgs),

    /// Edit an operation
    Edit { operation_id: u64 },

    /// Start an operation
    Start { operation_id: u64 },

    /// Pause an operation
    Pause { operation_id: u64 },

    /// Finish an operation successfully
    Complete { operation_id: u64 },

    /// Finish an operation unsuccessfully
    Abort { operation_id: u64 },

    /// Cancel an operation before starting
    Cancel { operation_id: u64 },

    /// Subscribe to an operation, component, or tag
    Subscribe(SubscribeArgs),

    /// Watch notifications
    Watch,

    /// Manage components
    Component {
        #[command(subcommand)]
        command: ComponentCommand,
    },

    /// Manage tags
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },

    /// Authenticate with the server
    Auth {
        #[arg(short, long)]
        username: String,
    },
}

async fn extract_result<T: DeserializeOwned>(response: Response) -> anyhow::Result<T> {
    let response: ApiResponse<T> = response.json().await?;
    match response {
        ApiResponse::Ok(value) => Ok(value),
        ApiResponse::Err(e) => anyhow::bail!("Error: {}", e),
    }
}

async fn print_response<T: Serialize + DeserializeOwned>(response: Response) -> anyhow::Result<()> {
    let value: T = extract_result(response).await?;
    let ps = SyntaxSet::load_defaults_nonewlines();
    let syntax = ps
        .find_syntax_by_name("YAML")
        .ok_or_else(|| anyhow::anyhow!("YAML syntax not found"))?;
    let ts = ThemeSet::load_defaults();
    let theme = ts
        .themes
        .get("base16-mocha.dark")
        .or_else(|| ts.themes.values().next())
        .ok_or_else(|| anyhow::anyhow!("no themes found"))?;
    let mut h = syntect::easy::HighlightLines::new(syntax, theme);
    let s = serde_yaml::to_string(&value)?;
    let mut stdout = std::io::stdout().lock();
    for line in s.lines() {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps)?;
        let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
        stdout.write_all(escaped.as_bytes())?;
        stdout.write_all(b"\n")?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let home = std::env::var("HOME")?;
    let app_dir = Path::new(&home).join(".smokestack");

    let api_root = cli.endpoint.join("/api/v1/")?;
    let token = match std::fs::read_to_string(app_dir.join("token")) {
        Ok(token) => token,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let Command::Auth { username } = cli.command else {
                anyhow::bail!("not authenticated; run `smokestack auth`");
            };
            let request = AuthRequest { username };
            let response = reqwest::Client::new()
                .post(api_root.join("auth")?)
                .json(&request)
                .send()
                .await?;
            let response: AuthResponse = extract_result(response).await?;
            let path = app_dir.join("token");
            std::fs::write(path, response.token)?;
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    let mut validation = jsonwebtoken::Validation::default();
    validation.insecure_disable_signature_validation();
    let key = jsonwebtoken::DecodingKey::from_secret(&[]);
    let token_data = jsonwebtoken::decode(&token, &key, &validation)?;
    let Claims { username, .. } = token_data.claims;

    let authorization = (
        reqwest::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}"))?,
    );
    let client = reqwest::ClientBuilder::new()
        .default_headers(HeaderMap::from_iter([authorization.clone()]))
        .build()?;

    match cli.command {
        Command::Create(args) => args.invoke(&client, &api_root, &app_dir, &username).await?,
        Command::Show { operation_id } => {
            let response = client
                .get(api_root.join(&format!("operations/{operation_id}"))?)
                .send()
                .await?;
            print_response::<Operation>(response).await?;
        }
        Command::List(args) => args.invoke(&client, &api_root).await?,
        Command::Edit { operation_id } => {
            let response = client
                .get(api_root.join(&format!("operations/{operation_id}"))?)
                .send()
                .await?;
            let operation: Operation = extract_result(response).await?;
            let mut request = UpdateOperationRequest {
                title: Some(operation.title),
                purpose: Some(operation.purpose),
                url: Some(operation.url),
                components: Some(operation.components),
                locks: Some(operation.locks),
                tags: Some(operation.tags),
                depends_on: Some(operation.depends_on),
                operators: Some(operation.operators),
                status: Some(operation.status),
                annotations: operation.annotations,
            };
            let content = serde_yaml::to_string(&request)?;
            request = serde_yaml::from_slice(&edit_yaml(&content)?)?;
            let response = client
                .patch(api_root.join(&format!("operations/{operation_id}"))?)
                .json(&request)
                .send()
                .await?;
            print_response::<Operation>(response).await?;
        }
        Command::Start { operation_id }
        | Command::Pause { operation_id }
        | Command::Complete { operation_id }
        | Command::Abort { operation_id }
        | Command::Cancel { operation_id } => {
            let new_status = match &cli.command {
                Command::Start { .. } => OperationState::InProgress,
                Command::Pause { .. } => OperationState::Paused,
                Command::Complete { .. } => OperationState::Completed,
                Command::Abort { .. } => OperationState::Aborted,
                Command::Cancel { .. } => OperationState::Canceled,
                _ => unreachable!(),
            };
            let request = UpdateOperationRequest {
                status: Some(new_status),
                ..Default::default()
            };
            let response = client
                .patch(api_root.join(&format!("operations/{operation_id}"))?)
                .json(&request)
                .send()
                .await?;
            print_response::<Operation>(response).await?;
        }
        Command::Subscribe(args) => args.invoke(&client, &api_root).await?,
        Command::Watch => subscription::watch(&client, &api_root, authorization).await?,
        Command::Component { command } => command.invoke(&client, &api_root).await?,
        Command::Tag { command } => command.invoke(&client, &api_root).await?,
        Command::Auth { .. } => anyhow::bail!("already authenticated as {}", username),
    }
    Ok(())
}

fn edit_yaml<T: AsRef<[u8]>>(s: T) -> anyhow::Result<Vec<u8>> {
    let mut file = tempfile::Builder::new().suffix(".yml").tempfile()?;
    file.write_all(s.as_ref())?;
    edit_file(file.path())?;
    std::fs::read(file.path()).map_err(Into::into)
}

fn edit_file<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let editor = editor()?;
    let status = std::process::Command::new(editor)
        .arg(path.as_ref())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    anyhow::ensure!(status.success(), "editor exited with status: {}", status);
    Ok(())
}

fn editor() -> anyhow::Result<OsString> {
    for var in ["VISUAL", "EDITOR"] {
        if let Some(editor) = std::env::var_os(var) {
            return Ok(editor);
        }
    }
    anyhow::bail!("neither VISUAL nor EDITOR are set");
}

fn colorize_status(status: OperationState) -> String {
    let color = match status {
        OperationState::Planned => 37,    // white
        OperationState::InProgress => 36, // cyan
        OperationState::Paused => 35,     // magenta
        OperationState::Completed => 32,  // green
        OperationState::Aborted => 31,    // red
        OperationState::Canceled => 33,   // yellow
    };
    format!("\x1b[{color}m{status}\x1b[0m")
}
