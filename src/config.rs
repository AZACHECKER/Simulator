use dotenvy::dotenv;
use std::env;

use crate::structs::Config;

macro_rules! get_env {
    ($name:expr) => {
        env::var($name).ok().filter(|k| !k.is_empty())
    };
    ($name:expr, $default:expr) => {
        env::var($name)
            .unwrap_or($default.to_string())
            .parse()
            .unwrap_or($default)
    };
}

pub fn config() -> Config {
    dotenv().ok();

    load_config()
}

fn load_config() -> Config {
    Config {
        port: get_env!("PORT", 8080),
        fork_url: get_env!("FORK_URL"),
        etherscan_key: get_env!("ETHERSCAN_KEY"),
        api_key: get_env!("API_KEY"),
        max_request_size: get_env!("MAX_REQUEST_SIZE", 16) * 1024,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_config_fork_url() {
        temp_env::with_vars([("FORK_URL", Some("a"))], || {
            let config = super::load_config();
            assert_eq!(config.fork_url, Some("a".to_string()));
        });

        temp_env::with_vars([("FORK_URL", Some(""))], || {
            let config = super::load_config();
            assert_eq!(config.fork_url, None);
        });

        temp_env::with_vars_unset(["FORK_URL"], || {
            let config = super::load_config();
            assert_eq!(config.fork_url, None);
        });
    }

    #[test]
    fn test_config_etherscan_key() {
        temp_env::with_vars([("ETHERSCAN_KEY", Some("a"))], || {
            let config = super::load_config();
            assert_eq!(config.etherscan_key, Some("a".to_string()));
        });

        temp_env::with_vars([("ETHERSCAN_KEY", Some(""))], || {
            let config = super::load_config();
            assert_eq!(config.etherscan_key, None);
        });

        temp_env::with_vars_unset(["ETHERSCAN_KEY"], || {
            let config = super::load_config();
            assert_eq!(config.etherscan_key, None);
        });
    }

    #[test]
    fn test_config_api_key() {
        temp_env::with_vars([("API_KEY", Some("a"))], || {
            let config = super::load_config();
            assert_eq!(config.api_key, Some("a".to_string()));
        });

        temp_env::with_vars([("API_KEY", Some(""))], || {
            let config = super::load_config();
            assert_eq!(config.api_key, None);
        });

        temp_env::with_vars_unset(["API_KEY"], || {
            let config = super::load_config();
            assert_eq!(config.api_key, None);
        });
    }
}
