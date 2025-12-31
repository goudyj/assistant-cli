#[cfg(test)]
use wiremock::matchers::{body_json, method, path};
#[cfg(test)]
use wiremock::{Mock, MockServer, ResponseTemplate};

#[cfg(test)]
pub struct MockChatServer {
    pub server: MockServer,
    pub endpoint: String,
}

#[cfg(test)]
impl MockChatServer {
    pub async fn new() -> Self {
        let server = MockServer::start().await;
        let endpoint = format!("{}/api/chat", server.uri());
        Self { server, endpoint }
    }

    pub async fn expect_json(
        &self,
        expected_body: serde_json::Value,
        response_body: serde_json::Value,
    ) {
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&self.server)
            .await;
    }

    pub async fn expect_status(
        &self,
        expected_body: serde_json::Value,
        status: u16,
        response_body: serde_json::Value,
    ) {
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(status).set_body_json(&response_body))
            .mount(&self.server)
            .await;
    }

    /// Mount a flexible mock that accepts any POST to /api/chat and returns the given response
    pub async fn expect_any(&self, response_body: serde_json::Value) {
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&self.server)
            .await;
    }
}
