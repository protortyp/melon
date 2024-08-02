use crate::helpers::spawn_app;

#[tokio::test]
async fn worker_registration_works() {
    let app = spawn_app().await;
    let res = app.register_node().await;
    assert!(res.is_ok())
}

#[tokio::test]
async fn worker_heartbeat_works() {
    let app = spawn_app().await;
    let res = app.register_node().await.unwrap();
    let res = res.get_ref();
    let node_id = res.node_id.clone();
    let res = app.send_heartbeat(node_id).await;
    assert!(res.is_ok())
}

#[tokio::test]
async fn worker_heartbeat_rejects_unknown_node() {
    let app = spawn_app().await;
    let node_id = String::from("UNKNOWN");
    let res = app.send_heartbeat(node_id).await;
    assert!(res.is_err())
}
