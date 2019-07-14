use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserConfig {
    pub profile: Profile,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    pub client_id: String,
    pub client_secret: String,
}

