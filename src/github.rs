use reqwest::blocking::Client;
use serde::Deserialize;
use anyhow::Result;

#[derive(Deserialize, Debug)]
pub struct TreeItem {
    pub path: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub sha: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct TreeResponse {
    pub tree: Vec<TreeItem>,
}

pub struct GitHubClient {
    client: Client,
    owner: String,
    repo: String,
}

impl GitHubClient {
    pub fn new(owner: &str, repo: &str) -> Self {
        let client = Client::builder()
            .user_agent("cb_patcher")
            .build()
            .unwrap();
        Self {
            client,
            owner: owner.to_string(),
            repo: repo.to_string(),
        }
    }

    pub fn fetch_tree(&self, branch: &str) -> Result<Vec<TreeItem>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
            self.owner, self.repo, branch
        );
        let resp: TreeResponse = self.client.get(&url).send()?.json()?;
        Ok(resp.tree)
    }

    pub fn download_file(&self, url: &str) -> Result<Vec<u8>> {
        // The tree item URL is for the blob API, which returns JSON with base64 content or raw.
        // However, for downloading, it's often easier to use the raw.githubusercontent.com URL 
        // or the "Accept: application/vnd.github.v3.raw" header on the blob URL.
        // Let's use the blob URL with the raw header.
        let resp = self.client.get(url)
            .header("Accept", "application/vnd.github.v3.raw")
            .send()?;
        Ok(resp.bytes()?.to_vec())
    }
}
