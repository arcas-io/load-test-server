use dotenv::dotenv;
use lazy_static::lazy_static;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub host: String,
    pub port: String,
    pub statsd_host: String,
    pub statsd_port: String,
}

// put the Config struct into a singleton CONFIG lazy_static
lazy_static! {
    pub static ref CONFIG: Config = get_config();
}

/// Use envy to deserialize environment variables into the Config struct
fn get_config() -> Config {
    dotenv().ok();

    envy::from_env::<Config>().unwrap_or_else(|error| panic!("Configuration Error: {:#?}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_gets_a_config() {
        let host = "123";
        std::env::set_var("HOST", host.to_string());
        let config = &CONFIG;
        assert_eq!(config.host, host.to_string());
    }
}
