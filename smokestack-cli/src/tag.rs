use crate::print_response;
use clap::Subcommand;
use reqwest::{Client, Url};
use smokestack::{
    api::{CreateTagRequest, ListTagsResponse},
    model::Tag,
};

#[derive(Debug, Subcommand)]
pub enum TagCommand {
    /// Create a new tag
    Create {
        name: String,

        #[arg(short, long)]
        description: String,
    },

    /// Show a tag
    Show { name: String },

    /// List tags
    List,
}

impl TagCommand {
    pub async fn invoke(self, client: &Client, api_root: &Url) -> anyhow::Result<()> {
        match self {
            Self::Create { name, description } => {
                let request = CreateTagRequest { name, description };
                let response = client
                    .post(api_root.join("tags")?)
                    .json(&request)
                    .send()
                    .await?;
                print_response::<Tag>(response).await?;
            }
            Self::Show { name } => {
                let response = client
                    .get(api_root.join(&format!("tags/{name}"))?)
                    .send()
                    .await?;
                print_response::<Tag>(response).await?;
            }
            Self::List => {
                let response = client.get(api_root.join("tags")?).send().await?;
                print_response::<ListTagsResponse>(response).await?;
            }
        }
        Ok(())
    }
}
