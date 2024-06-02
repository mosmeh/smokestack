use crate::print_response;
use clap::Subcommand;
use reqwest::{Client, Url};
use smokestack::{
    api::{CreateComponentRequest, ListComponentsResponse},
    model::Component,
};

#[derive(Debug, Subcommand)]
pub enum ComponentCommand {
    /// Create a new component
    Create {
        name: String,

        #[arg(short, long)]
        description: String,

        #[arg(short, long, alias = "owner", num_args = 1..)]
        owners: Vec<String>,
    },

    /// Show a component
    Show { name: String },

    /// List components
    List,
}

impl ComponentCommand {
    pub async fn invoke(self, client: &Client, api_root: &Url) -> anyhow::Result<()> {
        match self {
            Self::Create {
                name,
                description,
                owners,
            } => {
                let request = CreateComponentRequest {
                    name,
                    description,
                    owners,
                };
                let response = client
                    .post(api_root.join("components")?)
                    .json(&request)
                    .send()
                    .await?;
                print_response::<Component>(response).await?;
            }
            Self::Show { name } => {
                let response = client
                    .get(api_root.join(&format!("components/{name}"))?)
                    .send()
                    .await?;
                print_response::<Component>(response).await?;
            }
            Self::List => {
                let response = client.get(api_root.join("components")?).send().await?;
                print_response::<ListComponentsResponse>(response).await?;
            }
        }
        Ok(())
    }
}
