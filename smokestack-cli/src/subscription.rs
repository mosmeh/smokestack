use std::io::Write;

use crate::{colorize_status, extract_result, print_response};
use clap::{Args, Subcommand};
use futures_util::StreamExt;
use http::{HeaderName, HeaderValue};
use reqwest::{Client, Url};
use smokestack::{
    api::{CreateSubscriptionRequest, ListOperationsResponse, ListSubscriptionResponse},
    model::Operation,
};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct SubscribeArgs {
    /// List subscriptions
    #[arg(short, long)]
    list: bool,

    /// Operation ID to subscribe to
    #[arg(short, long)]
    operation: Option<u64>,

    /// Component name to subscribe to
    #[arg(short, long)]
    component: Option<String>,

    /// Tag name to subscribe to
    #[arg(short, long)]
    tag: Option<String>,
}

#[derive(Debug, Subcommand)]
enum SubscribeCommand {
    /// List subscriptions
    List,
}

impl SubscribeArgs {
    pub async fn invoke(self, client: &Client, api_root: &Url) -> anyhow::Result<()> {
        if self.list {
            let response = client.get(api_root.join("subscriptions")?).send().await?;
            print_response::<ListSubscriptionResponse>(response).await?;
        } else {
            let request = CreateSubscriptionRequest {
                operation: self.operation,
                component: self.component,
                tag: self.tag,
            };
            let response = client
                .post(api_root.join("subscriptions")?)
                .json(&request)
                .send()
                .await?;
            extract_result::<()>(response).await?;
        }
        Ok(())
    }
}

pub async fn watch(
    client: &Client,
    api_root: &Url,
    authorization: (HeaderName, HeaderValue),
) -> anyhow::Result<()> {
    fn print_operation<W: std::io::Write>(
        out: &mut W,
        operation: &Operation,
    ) -> std::io::Result<()> {
        write!(
            out,
            "{}  {:>9}  ",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), // Fake timestamp
            operation.id
        )?;
        out.write_all(colorize_status(operation.status).as_bytes())?;
        for _ in operation.status.to_string().len().."in_progress".len() {
            out.write_all(b" ")?;
        }
        writeln!(out, "  {}", operation.title)
    }

    let mut stdout = std::io::stdout();
    stdout.write_all(b"time                 operation  status       title\n-------------------  ---------  -----------  -----\n")?;

    // TODO: fetch history instead of the final states of the past operations
    let response = client.get(api_root.join("operations")?).send().await?;
    let ListOperationsResponse { operations } = extract_result(response).await?;
    for operation in operations {
        print_operation(&mut stdout, &operation)?;
    }

    let mut url = api_root.join("subscriptions/watch")?;
    url.set_scheme("ws").unwrap();
    let mut request = url.into_client_request()?;
    request.headers_mut().extend([authorization]);
    let (mut stream, _) = tokio_tungstenite::connect_async(request).await?;
    while let Some(msg) = stream.next().await {
        let tokio_tungstenite::tungstenite::Message::Text(msg) = msg? else {
            anyhow::bail!("unexpected message type");
        };
        let operation: Operation = serde_json::from_str(&msg)?;
        print_operation(&mut stdout, &operation)?;
    }
    Ok(())
}
