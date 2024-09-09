use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
pub use controller::{self, telemetry, State};
use prometheus::{Encoder, TextEncoder};

#[get("/metrics")]
async fn metrics(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    encoder.encode(&metrics, &mut buffer).unwrap();
    HttpResponse::Ok().body(buffer)
}

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/")]
async fn index(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init().await;

    // Init k8s controller state
    let state = State::new();
    let fleet_config_controller = controller::run_fleet_addon_config_controller(state.clone());
    let cluster_controller = controller::run_cluster_controller(state.clone());
    let cluster_class_controller = controller::run_cluster_class_controller(state.clone());

    // Start web server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(index)
            .service(health)
            .service(metrics)
    })
    .bind("0.0.0.0:8443")?
    .shutdown_timeout(5)
    .run();

    tokio::join!(
        cluster_controller,
        cluster_class_controller,
        fleet_config_controller,
        server
    )
    .3?;
    Ok(())
}
