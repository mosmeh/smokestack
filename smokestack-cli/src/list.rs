use crate::{colorize_status, extract_result};
use clap::Args;
use reqwest::{Client, Url};
use smokestack::{api::ListOperationsResponse, model::OperationState};
use std::io::Write;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(short, long = "component", name = "COMPONENT", num_args = 1..)]
    components: Vec<String>,

    #[arg(short, long = "tag", name = "TAG", num_args = 1..)]
    tags: Vec<String>,

    #[arg(short, long = "status", name = "STATUS", num_args = 1..)]
    statuses: Vec<OperationState>,

    #[arg(short, long = "operator", name = "OPERATOR", num_args = 1..)]
    operators: Vec<String>,
}

impl ListArgs {
    pub async fn invoke(self, client: &Client, api_root: &Url) -> anyhow::Result<()> {
        let mut query = Vec::new();
        for component in self.components {
            query.push(("component", component));
        }
        for tag in self.tags {
            query.push(("tag", tag));
        }
        for operator in self.operators {
            query.push(("operator", operator));
        }
        for status in self.statuses {
            query.push(("status", status.to_string()));
        }
        let response = client
            .get(api_root.join("operations")?)
            .query(&query)
            .send()
            .await?;
        let ListOperationsResponse { mut operations } = extract_result(response).await?;
        let mut max_id_width = "id".len();
        let mut max_status_width = "status".len();
        let mut max_title_width = "title".len();
        for operation in &operations {
            max_id_width = max_id_width.max(operation.id.to_string().len());
            max_status_width = max_status_width.max(operation.status.to_string().len());
            max_title_width = max_title_width.max(operation.title.width());
        }
        let mut stdout = std::io::stdout().lock();
        writeln!(
            &mut stdout,
            "{:>id_width$}  {:status_width$}  {:title_width$}",
            "id",
            "status",
            "title",
            id_width = max_id_width,
            status_width = max_status_width,
            title_width = max_title_width
        )?;
        for _ in 0..max_id_width {
            stdout.write_all(b"-")?;
        }
        stdout.write_all(b"  ")?;
        for _ in 0..max_status_width {
            stdout.write_all(b"-")?;
        }
        stdout.write_all(b"  ")?;
        for _ in 0..max_title_width {
            stdout.write_all(b"-")?;
        }
        stdout.write_all(b"\n")?;
        operations.reverse();
        for operation in operations {
            write!(
                &mut stdout,
                "{:>width$}  ",
                operation.id,
                width = max_id_width
            )?;
            write!(&mut stdout, "{}  ", colorize_status(operation.status))?;
            for _ in operation.status.to_string().len()..max_status_width {
                stdout.write_all(b" ")?;
            }
            stdout.write_all(operation.title.as_bytes())?;
            stdout.write_all(b"\n")?;
        }
        Ok(())
    }
}
