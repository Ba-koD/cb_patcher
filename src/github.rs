use reqwest::blocking::Client;
use serde::Deserialize;
use anyhow::Result;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct TreeItem {
    pub path: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub sha: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct TreeResponse {
    pub tree: Vec<TreeItem>,
}

#[derive(Clone)]
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

    #[allow(dead_code)]
    pub fn fetch_tree(&self, branch: &str) -> Result<Vec<TreeItem>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
            self.owner, self.repo, branch
        );
        let resp = self.client.get(&url).send()?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            if status.as_u16() == 403 && text.contains("rate limit") {
                return Err(anyhow::anyhow!("GitHub API Rate Limit Exceeded. Please try again later."));
            }
            return Err(anyhow::anyhow!("GitHub API Error {}: {}", status, text));
        }

        let resp: TreeResponse = resp.json()?;
        Ok(resp.tree)
    }

    #[allow(dead_code)]
    pub fn download_file(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.client.get(url)
            .header("Accept", "application/vnd.github.v3.raw")
            .send()?;
        Ok(resp.bytes()?.to_vec())
    }

    pub fn download_repo_zip(&self, branch: &str) -> Result<Vec<u8>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/zipball/{}",
            self.owner, self.repo, branch
        );
        let resp = self.client.get(&url).send()?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            if status.as_u16() == 403 && text.contains("rate limit") {
                return Err(anyhow::anyhow!("GitHub API Rate Limit Exceeded. Please try again later."));
            }
             return Err(anyhow::anyhow!("Failed to download zip: {} - {}", status, text));
        }
        Ok(resp.bytes()?.to_vec())
    }

    pub fn fetch_metadata_id(&self, branch: &str) -> Result<String> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/metadata.xml",
            self.owner, self.repo, branch
        );
        let content = self.client.get(&url).send()?.text()?;
        
        #[derive(Deserialize)]
        struct Metadata {
            id: String,
        }
        
        let metadata: Metadata = quick_xml::de::from_str(&content)?;
        Ok(metadata.id)
    }
}
