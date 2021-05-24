use crate::email_client::EmailClient;
use crate::routes;
use actix_web::dev::Server;
// use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

use crate::{configuration, email_client};

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: configuration::Settings) -> Result<Self, std::io::Error> {
        let connection_pool = get_connection_pool(&configuration.database)
            .await
            .expect("Failed to connect to Postgres.");

        let sender_email = configuration
            .email_client
            .sender()
            .expect("invalid sender email address");
        let email_client = email_client::EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
        );

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port,
        );

        let listener = TcpListener::bind(&address)?;
        let port = listener.local_addr().unwrap().port();
        // let address = if configuration.application.port == 0 {
        //     listener.local_addr().unwrap().to_string()
        // } else {
        //   address
        // };
        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
        )?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub async fn get_connection_pool(
    configuration: &configuration::DatabaseSettings,
) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_with(configuration.with_db())
        .await
}

pub struct ApplicationBaseUrl(pub String);

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
) -> Result<Server, std::io::Error> {
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger)
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .route("/subscriptions/confirm", web::get().to(routes::confirm))
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .data(ApplicationBaseUrl(base_url.clone()))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
