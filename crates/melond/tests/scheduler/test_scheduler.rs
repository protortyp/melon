use crate::helpers::{get_node_info, spawn_app};
use crate::mock_worker::MockWorker;
use melon_common::proto;
use melon_common::proto::melon_worker_server::MelonWorkerServer;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self};
use tokio::sync::watch;
use tonic::transport::Server;

const TEST_MEMORY_SIZE: u64 = 2 * 1024 * 1024;
const TEST_COU_COUNT: u32 = 1;
const TEST_TIME_MINS: u32 = 1024;
const TEST_SCRIPT_PATH: &str = "/path/to/script";
const TEST_USER: &str = "chris";

#[tokio::test]
async fn worker_registration_works() {
    let app = spawn_app().await;
    let res = app.register_node(get_node_info(42)).await;
    assert!(res.is_ok())
}

#[tokio::test]
async fn worker_heartbeat_works() {
    let app = spawn_app().await;
    let res = app.register_node(get_node_info(42)).await.unwrap();
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

#[tokio::test]
async fn submit_job_works() {
    let app = spawn_app().await;
    let submission = get_job_submission();

    let res = app.submit_job(submission).await;

    assert!(res.is_ok())
}

#[tokio::test]
async fn test_list_pending_job() {
    let app = spawn_app().await;
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;

    let res = app.list_jobs().await.unwrap();
    let res = res.get_ref();
    let first_job = res.jobs.first().unwrap();

    assert_eq!(first_job.job_id, job_id);
    assert_eq!(first_job.user, submission.user);
    assert_eq!(first_job.status, "PD".to_string());
}

#[tokio::test]
async fn test_list_running_job() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let res = app.list_jobs().await.unwrap();
    let res = res.get_ref();
    let first_job = res.jobs.first().unwrap();

    assert_eq!(first_job.job_id, job_id);
    assert_eq!(first_job.user, submission.user);
    assert_eq!(first_job.status, "R".to_string());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

struct MockWorkerSetup {
    job_assignment_receiver: mpsc::Receiver<proto::JobAssignment>,
    job_cancellation_receiver: mpsc::Receiver<proto::CancelJobRequest>,
    server_notifier: watch::Sender<()>,
    server_handle: tokio::task::JoinHandle<()>,
    job_extension_receiver: mpsc::Receiver<proto::ExtendJobRequest>,
    port: u16,
}

async fn setup_mock_worker() -> MockWorkerSetup {
    let (job_assignment_sender, job_assignment_receiver) = mpsc::channel(1);
    let (job_cancellation_sender, job_cancellation_receiver) = mpsc::channel(1);
    let (server_notifier, server_notifier_rx) = watch::channel(());
    let (job_extension_sender, job_extension_receiver) = mpsc::channel(1);

    let worker = MockWorker::new(
        job_assignment_sender.clone(),
        job_cancellation_sender.clone(),
        job_extension_sender.clone(),
    )
    .await
    .unwrap();

    let addr = String::from("[::1]:0");
    let listener = TcpListener::bind(&addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let mut shutdown_rx = server_notifier_rx.clone();

    let server_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(MelonWorkerServer::new(worker))
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                async {
                    shutdown_rx.changed().await.ok();
                },
            )
            .await
            .unwrap();
    });

    MockWorkerSetup {
        job_assignment_receiver,
        job_cancellation_receiver,
        server_notifier,
        server_handle,
        job_extension_receiver,
        port,
    }
}

#[tokio::test]
async fn test_successful_job_assignment() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();

    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let job_response = res.get_ref();
    let job_assignment = mock_setup.job_assignment_receiver.recv().await.unwrap();

    assert_eq!(job_response.job_id, job_assignment.job_id);
    assert_eq!(submission.req_res, job_assignment.req_res);

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_submit_job_results() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let job_assignment = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let job_result = proto::JobResult {
        job_id: job_assignment.job_id,
        status: 1,
        message: "".to_string(),
    };
    let res = app.submit_job_result(job_result).await;
    assert!(res.is_ok());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_submit_job_fails_for_unknown_id() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let job_result = proto::JobResult {
        job_id: 99999999,
        status: 1,
        message: "".to_string(),
    };
    let res = app.submit_job_result(job_result).await;
    assert!(res.is_err());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_cancel_pending_job_successfully() {
    let app = spawn_app().await;
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;

    let request = proto::CancelJobRequest {
        job_id,
        user: TEST_USER.to_string(),
    };
    let res = app.cancel_job(request).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_cancel_pending_job_fails_unauthorized() {
    let app = spawn_app().await;
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;

    let request = proto::CancelJobRequest {
        job_id,
        user: "RANDOM USER".to_string(),
    };
    let res = app.cancel_job(request).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_cancel_running_job() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let request = proto::CancelJobRequest {
        job_id,
        user: TEST_USER.to_string(),
    };
    let res = app.cancel_job(request).await;
    let cancel_request = mock_setup.job_cancellation_receiver.recv().await.unwrap();

    assert!(res.is_ok());
    assert_eq!(cancel_request.job_id, job_id);
    assert_eq!(cancel_request.user, TEST_USER.to_string());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_reject_running_job_cancellation_with_incorrect_user() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let request = proto::CancelJobRequest {
        job_id,
        user: "UNKNOWN".to_string(),
    };
    let res = app.cancel_job(request).await;
    assert!(res.is_err());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_reject_unknown_cancel_request() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let request = proto::CancelJobRequest {
        job_id: 9999000,
        user: TEST_USER.to_string(),
    };
    let res = app.cancel_job(request).await;
    assert!(res.is_err());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_extend_pending_job() {
    let app = spawn_app().await;
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();

    let request = proto::ExtendJobRequest {
        job_id: res.job_id,
        user: TEST_USER.to_string(),
        extension_mins: 125,
    };
    let res = app.extend_job(request).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_extend_running_job() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let request = proto::ExtendJobRequest {
        job_id,
        user: TEST_USER.to_string(),
        extension_mins: 125,
    };
    let _ = app.extend_job(request).await.unwrap();
    let request = mock_setup.job_extension_receiver.recv().await.unwrap();

    assert_eq!(request.extension_mins, 125);
    assert_eq!(request.job_id, job_id);
    assert_eq!(request.user, TEST_USER.to_string());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_reject_unauthorized_extension_pending() {
    let app = spawn_app().await;
    let mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;

    let request = proto::ExtendJobRequest {
        job_id,
        user: "UNKNOWN".to_string(),
        extension_mins: 125,
    };
    let res = app.extend_job(request).await;

    assert!(res.is_err());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_reject_unauthorized_extension_running() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let request = proto::ExtendJobRequest {
        job_id,
        user: "UNKNOWN".to_string(),
        extension_mins: 125,
    };
    let res = app.extend_job(request).await;

    assert!(res.is_err());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_reject_unknown_extension_for_pending() {
    let app = spawn_app().await;
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();

    let request = proto::ExtendJobRequest {
        job_id: 99999,
        user: TEST_USER.to_string(),
        extension_mins: 125,
    };
    let res = app.extend_job(request).await;

    assert!(res.is_err());
}

#[tokio::test]
async fn test_reject_unknown_extension_for_running() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let _ = mock_setup.job_assignment_receiver.recv().await.unwrap();

    let request = proto::ExtendJobRequest {
        job_id: 99999,
        user: TEST_USER.to_string(),
        extension_mins: 125,
    };
    let res = app.extend_job(request).await;

    assert!(res.is_err());

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_reject_unknown_extension() {
    let app = spawn_app().await;

    let request = proto::ExtendJobRequest {
        job_id: 99999,
        user: TEST_USER.to_string(),
        extension_mins: 125,
    };
    let res = app.extend_job(request).await;

    assert!(res.is_err());
}

fn get_job_submission() -> proto::JobSubmission {
    proto::JobSubmission {
        user: TEST_USER.to_string(),
        script_path: TEST_SCRIPT_PATH.to_string(),
        req_res: Some(proto::Resources {
            cpu_count: TEST_COU_COUNT,
            memory: TEST_MEMORY_SIZE,
            time: TEST_TIME_MINS,
        }),
        script_args: [].to_vec(),
    }
}
