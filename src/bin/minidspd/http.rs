//! A Hello World example application for working with Gotham.
use gotham::{
    handler::{HandlerError, IntoResponse},
    helpers::http::response::create_response,
    hyper::StatusCode,
    router::{
        builder::{build_simple_router, DefineSingleRoute, DrawRoutes},
        Router,
    },
    state::State,
};
use mime;
use minidsp::MiniDSP;

pub async fn master_status(state: &mut State) -> Result<impl IntoResponse, HandlerError> {
    let app = super::APP.clone();
    let app = app.read().await;
    let devices = app.devices.read().await;
    if !devices.is_empty() {
        let dev = devices.get(0).unwrap();

        let service = dev.service.try_read();
        let service = service.unwrap().as_ref().unwrap().clone();

        let dsp = MiniDSP::new(service.clone(), &minidsp::device::DEVICE_2X4HD);
        let status = dsp.get_master_status().await?;

        let res = create_response(
            &state,
            StatusCode::OK,
            mime::APPLICATION_JSON,
            serde_json::to_vec(&status).unwrap(),
        );
        return Ok(res);
    }

    let res = create_response(&state, StatusCode::NOT_FOUND, mime::APPLICATION_JSON, "");
    Ok(res)
}

/// Create a `Router`.
fn router() -> Router {
    build_simple_router(|route| {
        route.get("/").to_async_borrowing(master_status);
    })
}

/// Start a server and call the `Handler` we've defined above for each `Request` we receive.
pub async fn main() {
    let addr = "127.0.0.1:7878";
    println!("Listening for requests at http://{}", addr);
    // tokio::task::block_in_place(|| {
    //     gotham::start(addr, || Ok(master_status));
    // });

    let _ = gotham::init_server(addr, router()).await;
}
