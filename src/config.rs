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
impl Profile {
    fn new() -> Self {
        Profile {client_id:"".to_string(), client_secret:"".to_string()}
    }
}

impl UserConfig {
    pub fn new() -> Self {
        UserConfig {profile: Profile::new()}
    }
    pub fn client_id(mut self, client_id: String) -> Self {
        self.profile.client_id = client_id;
        self
    }
    pub fn client_secret(mut self, client_secret: String) -> Self {
        self.profile.client_secret = client_secret;
        self
    }
}

