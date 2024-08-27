use crate::{
    constants::*,
    helpers::{get_job_submission, get_node_info, spawn_app, TestApp},
    mock_worker::setup_mock_worker,
};
use serde_json::Value;

#[tokio::test]
async fn test_api_list_jobs() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();

    // submit jobs and wait for assignments
    let job_ids = submit_multiple_jobs(&app, 1).await;
    for _ in 0..1 {
        let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();
    }

    // list jobs from api
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}:{}/api/jobs", app.api_host, app.api_port))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let jobs: Vec<Value> = response.json().await.unwrap();

    assert_eq!(jobs.len(), 1);
    for (index, job) in jobs.iter().enumerate() {
        assert_eq!(job["id"].as_u64().unwrap(), job_ids[index] as u64);
        assert_eq!(job["user"].as_str().unwrap(), TEST_USER);
        assert_eq!(job["status"].as_str().unwrap(), "Running");
    }

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

async fn submit_multiple_jobs(app: &TestApp, count: usize) -> Vec<u64> {
    let mut job_ids = Vec::new();
    for _ in 0..count {
        let submission = get_job_submission();
        let res = app.submit_job(submission).await.unwrap();
        let res = res.get_ref();
        job_ids.push(res.job_id);
    }
    job_ids
}
