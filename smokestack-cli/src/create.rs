use crate::{edit_yaml, print_response};
use anyhow::Context;
use clap::Args;
use http::Uri;
use reqwest::{Client, Url};
use serde::Serialize;
use smokestack::{api::CreateOperationRequest, model::Operation};
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    str::FromStr,
};

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[command(flatten)]
    source: SourceArgs,

    /// Open an editor to edit the operation description before submission.
    #[arg(short, long)]
    edit: bool,

    /// Do not open an editor to edit the operation description before
    /// submission.
    #[arg(long, conflicts_with = "edit", requires = "source")]
    no_edit: bool,
}

#[derive(Debug, Args)]
#[group(id = "source")]
struct SourceArgs {
    /// Read an operation description from a file. Use `-` to read from stdin.
    #[arg(
        short,
        long,
        conflicts_with = "importer",
        conflicts_with = "template",
        conflicts_with = "KEY=VALUE"
    )]
    file: Option<Input>,

    #[arg(short, long, conflicts_with = "template")]
    importer: Option<String>,

    #[arg(short, long)]
    template: Option<String>,

    /// Parameters to provide to importers or templates
    #[arg(
        short,
        long = "parameter",
        alias = "param",
        alias = "params",
        name = "KEY=VALUE",
        value_parser = try_parse_key_val
    )]
    parameters: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
enum Input {
    Stdin,
    Path(PathBuf),
}

impl FromStr for Input {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s == "-" {
            Self::Stdin
        } else {
            Self::Path(s.into())
        })
    }
}

impl Input {
    fn read_to_end(&self) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Stdin => {
                let mut buf = Vec::new();
                std::io::stdin().lock().read_to_end(&mut buf)?;
                Ok(buf)
            }
            Self::Path(path) => std::fs::read(path).map_err(Into::into),
        }
    }
}

fn try_parse_key_val(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((key, val)) => Ok((key.to_string(), val.to_string())),
        None => Err(format!("invalid KEY=VALUE: no `=` found in `{s}`")),
    }
}

#[derive(Serialize)]
struct OperationCreation {
    title: String,
    purpose: String,
    #[serde(with = "smokestack::serde_uri")]
    url: Uri,
    components: Vec<String>,
    locks: Vec<String>,
    tags: Vec<String>,
    depends_on: Vec<u64>,
    operators: Vec<String>,
    annotations: HashMap<String, String>,
}

impl From<OperationCreation> for CreateOperationRequest {
    fn from(oc: OperationCreation) -> Self {
        Self {
            title: oc.title,
            purpose: oc.purpose,
            url: oc.url,
            components: oc.components,
            locks: oc.locks,
            tags: oc.tags,
            depends_on: oc.depends_on,
            operators: oc.operators,
            annotations: oc.annotations,
        }
    }
}

impl CreateArgs {
    pub async fn invoke(
        self,
        client: &Client,
        api_root: &Url,
        app_dir: &Path,
        username: &str,
    ) -> anyhow::Result<()> {
        let mut edit = self.edit;
        let mut content = match self.source {
            // Create a skeleton operation description.
            SourceArgs {
                file: None,
                importer: None,
                template: None,
                parameters,
            } if parameters.is_empty() => {
                edit = true;
                serde_yaml::to_string(&OperationCreation {
                    title: String::new(),
                    purpose: String::new(),
                    url: "http://example.com".parse().unwrap(),
                    components: Vec::new(),
                    locks: Vec::new(),
                    tags: Vec::new(),
                    depends_on: Vec::new(),
                    operators: vec![username.to_owned()],
                    annotations: HashMap::new(),
                })?
                .into_bytes()
            }

            // Read the operation description from a file or stdin.
            SourceArgs {
                file: Some(input),
                importer: None,
                template: None,
                parameters,
            } => {
                assert!(parameters.is_empty());
                input.read_to_end()?
            }

            // Use a template to generate the operation description.
            SourceArgs {
                file: None,
                importer: None,
                template: Some(template),
                parameters,
            } => {
                edit = true;
                let templates_dir = app_dir.join("templates");
                std::fs::create_dir_all(&templates_dir)?;
                let mut dir = templates_dir
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid template directory"))?
                    .to_owned();
                dir.push_str("/**/*");
                let tera = tera::Tera::new(&dir)?;
                let mut context = tera::Context::new();
                for (k, v) in parameters {
                    context.insert(k, &v);
                }
                tera.render(&template, &context)?.into_bytes()
            }

            // Run importers to generate the operation description.
            SourceArgs {
                file: None,
                importer,
                template: None,
                parameters,
            } => {
                let importers_path = app_dir.join("importers");
                std::fs::create_dir_all(&importers_path)?;
                let envs = parameters
                    .into_iter()
                    .map(|(k, v)| (format!("SMOKESTACK_PARAM_{}", k.to_uppercase()), v));
                let importer_paths = if let Some(importer) = importer {
                    vec![importers_path.join(importer)]
                } else {
                    std::fs::read_dir(importers_path)?
                        .map(|entry| entry.map(|e| e.path()))
                        .collect::<Result<Vec<_>, _>>()?
                };
                let mut outputs = importer_paths
                    .into_iter()
                    .map(|path| {
                        std::process::Command::new(path)
                            .envs(envs.clone())
                            .stderr(Stdio::inherit())
                            .output()
                    })
                    .filter(|output| {
                        // If the importer exits with status 125, skip it.
                        // Otherwise, the importer succeeded or failed, so we should return the output.
                        output
                            .as_ref()
                            .map_or(true, |o| o.status.code() != Some(125))
                    });
                let output = outputs
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("no matching importer found"))?
                    .with_context(|| "failed to run importer")?;
                anyhow::ensure!(
                    output.status.success(),
                    "importer exited with {}",
                    output.status
                );
                output.stdout
            }

            _ => unreachable!(),
        };

        if edit && !self.no_edit {
            content = edit_yaml(&content)?;
        }

        let request: CreateOperationRequest = serde_yaml::from_slice(&content)?;
        let response = client
            .post(api_root.join("operations")?)
            .json(&request)
            .send()
            .await?;
        print_response::<Operation>(response).await?;
        Ok(())
    }
}
