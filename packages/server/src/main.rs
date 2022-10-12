use std::{thread, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        TypedHeader,
    },
    headers,
    response::IntoResponse,
    routing::get,
    Router, Server,
};
use serpent_automation_executor::{
    library::Library,
    run::{run, RunTracer},
    syntax_tree::parse,
    CODE,
};
use tokio::{sync::watch, time::sleep};

#[tokio::main]
async fn main() {
    let lib = Library::link(parse(CODE).unwrap());

    let (trace_send, trace_receive) = watch::channel(RunTracer::new());

    thread::scope(|scope| {
        scope.spawn(|| server(trace_receive));
        scope.spawn(|| loop {
            run(&lib, &trace_send);
            thread::sleep(Duration::from_secs(3));
            trace_send.send_replace(RunTracer::new());
        });
    });
}

#[tokio::main]
async fn server(tracer: watch::Receiver<RunTracer>) {
    let handler =
        move |ws, user_agent| async move { upgrade_to_websocket(tracer, ws, user_agent).await };
    let app = Router::new().route("/", get(handler));
    Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn upgrade_to_websocket(
    tracer: watch::Receiver<RunTracer>,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        println!("`{}` connected", user_agent.as_str());
    }

    ws.on_upgrade(move |socket| handler(tracer, socket))
}

async fn handler(mut tracer: watch::Receiver<RunTracer>, mut socket: WebSocket) {
    println!("Upgraded to websocket");

    loop {
        tracer.changed().await.unwrap();

        let serialize_tracer = serde_json::to_string(&*tracer.borrow()).unwrap();
        println!("Sending run state");

        // TODO: Diff `RunTracer` and send a `RunTracerDelta`
        if socket.send(Message::Text(serialize_tracer)).await.is_err() {
            println!("Client disconnected");
            return;
        }

        sleep(Duration::from_millis(100)).await;
    }
}
