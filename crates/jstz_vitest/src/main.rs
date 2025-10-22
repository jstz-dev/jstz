use std::rc::Rc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::{routing::any, Router};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions};
use jstz_vitest::resolver::{create_deno_node_ext, create_node_resolver};
use jstz_vitest::transpile::transpile_extension_ts;
use jstz_vitest::MemoModuleLoader;
use url::Url;
#[tokio::main]
async fn main() {
    //   let app: Router<()> = Router::new().route("/", any(handler));

    //   let listener = tokio::net::TcpListener::bind("0.0.0.0:54322").await.unwrap();
    //   println!("Listening on 0.0.0.0:54322");
    //   axum::serve(listener, app).await.unwrap();

    let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
        module_loader: Rc::new(MemoModuleLoader {
            node_resolver: create_node_resolver(),
        }),
        extensions: create_deno_node_ext(),
        extension_transpiler: Some(Rc::new(transpile_extension_ts)),
        ..Default::default()
    });
    let specifier = Url::parse("file:///Users/ryan-tan/workspace/jstz/packages/vitest-env/build/tests/test_module_a.js").unwrap();
    let module_id = runtime.load_side_es_module(&specifier).await.unwrap();
    let _ = runtime.evaluate_module(module_id).await.unwrap();

    let obj = runtime.get_module_namespace(module_id).unwrap();
}

async fn handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        let msg = if let Ok(msg) = msg {
            println!("{msg:?}");
            msg
        } else {
            // client disconnected
            return;
        };

        if socket
            .send(Message::Text(format!("Echo: {}", msg.into_text().unwrap())))
            .await
            .is_err()
        {
            // client disconnected
            return;
        }
    }
}
