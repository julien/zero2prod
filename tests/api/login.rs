use crate::helpers::{assert_is_redirect_to, spawn_app};
use reqwest::header::HeaderValue;
use std::collections::HashSet;

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    // Arrange
    let app = spawn_app().await;

    // Act - try to login
    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password",
    });
    let response = app.post_login(&login_body).await;

    // Follow the redirect
    assert_is_redirect_to(&response, "/login");

    let html_page = app.get_login_html().await;
    assert!(html_page.contains("<p><i>authentication failed</i></p>"));

    // Reload the page again
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains("authentication failed"));
}