#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub fork_url: Option<String>,
    pub etherscan_key: Option<String>,
    pub api_key: Option<String>,
    pub max_request_size: u64,
}