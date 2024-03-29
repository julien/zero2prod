use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use actix_web::http::header::{HeaderMap, HeaderValue};
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use anyhow::Context;
use base64::Engine;
use secrecy::Secret;
use sqlx::PgPool;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_vaue = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();

                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_vaue);

                response
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

// #[tracing::instrument(name = "validate credentials", skip(credentials, pool))]
// async fn validate_credentials(
//     credentials: Credentials,
//     pool: &PgPool,
// ) -> Result<uuid::Uuid, AuthError> {
//     let mut user_id = None;
//     let mut expected_password_hash = Secret::new(
//         "$argon2id$v=19$m=15000,t=2,p=1$\
//         gZiV/M1gPc22ElAH/Jh1Hw$\
//         CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
//             .to_string(),
//     );

//     if let Some((stored_user_id, stored_password_hash)) =
//         get_stored_credentials(&credentials.username, &pool).await?
//     {
//         user_id = Some(stored_user_id);
//         expected_password_hash = stored_password_hash;
//     }

//     spawn_blocking_with_tracing(move || {
//         verify_password_hash(expected_password_hash, credentials.password)
//     })
//     .await
//     .context("failed to spawn blocking task")??;

//     user_id
//         .ok_or_else(|| anyhow::anyhow!("unknown username"))
//         .map_err(AuthError::InvalidCredentials)
// }

// #[tracing::instrument(
//     name = "verify password hash",
//     skip(expected_password_hash, password_candidate)
// )]
// fn verify_password_hash(
//     expected_password_hash: Secret<String>,
//     password_candidate: Secret<String>,
// ) -> Result<(), AuthError> {
//     let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
//         .context("failed to parse hash in PHC string format")?;

//     Argon2::default()
//         .verify_password(
//             password_candidate.expose_secret().as_bytes(),
//             &expected_password_hash,
//         )
//         .context("invalid password")
//         .map_err(AuthError::InvalidCredentials)
// }

// #[tracing::instrument(name = "get stored credentials", skip(username, pool))]
// async fn get_stored_credentials(
//     username: &str,
//     pool: &PgPool,
// ) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
//     let row: Option<_> = sqlx::query!(
//         r#"
//         SELECT user_id, password_hash
//         FROM users
//         WHERE username = $1
//         "#,
//         username,
//     )
//     .fetch_optional(pool)
//     .await
//     .context("failed to perform a query to retrieve stored credentials")?
//     .map(|row| (row.user_id, Secret::new(row.password_hash)));
//     Ok(row)
// }

#[tracing::instrument(
    name = "publish a newsletter issue",
    skip(body, pool, email_client, request),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = validate_credentials(credentials, &pool)
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => PublishError::AuthError(e.into()),
            AuthError::UnexpectedError(_) => PublishError::UnexpectedError(e.into()),
        })?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| {
                        format!("failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    // We record the error chain as a structured field
                    // on the log record.
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are valid",
                );
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

// struct Credentials {
//     username: String,
//     password: Secret<String>,
// }

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // The header value, if present, must be a valid UTF8 string
    let heade_value = headers
        .get("Authorization")
        .context("the 'Authorization' header was missing")?
        .to_str()
        .context("the 'Authorization' header was not a valid UTF8 string")?;
    let base64encoded_segment = heade_value
        .strip_prefix("Basic ")
        .context("the 'Authorization' scheme was not 'Basic'")?;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_segment)
        .context("failed to base64-decode the 'Basic' credentials")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("the decoded credentials string is not valid UTF8")?;

    // Split into segments, using ':' as a delimiter
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("a username must be provided in 'Basic' auth"))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("a password must be provided in 'Basic' auth"))?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
    // We are returning a Vec of Result in the happy case.
    // This allows the caller to bubble up errors due to network issues
    // or other failures using the ? operator.
    // See http://sled.rs/errors.html for a deep-dive about this technique.
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();

    Ok(confirmed_subscribers)
}
