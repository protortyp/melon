use crate::{
    constants::*,
    helpers::{get_job_submission, get_node_info, spawn_app},
    mock_worker::setup_mock_worker,
};
use melon_common::{proto, JobStatus};
use tonic::Status;

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

    assert_eq!(first_job.id, job_id);
    assert_eq!(first_job.user, submission.user);
    assert_eq!(JobStatus::from(first_job.status), JobStatus::Pending);
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

    assert_eq!(first_job.id, job_id);
    assert_eq!(first_job.user, submission.user);
    assert_eq!(JobStatus::from(first_job.status), JobStatus::Running);

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
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

#[tokio::test]
async fn test_mshow_pending() {
    let app = spawn_app().await;
    let mock_setup = setup_mock_worker().await;
    let submission = get_job_submission();
    let res = app.submit_job(submission.clone()).await.unwrap();
    let res = res.get_ref();
    let job_id = res.job_id;

    let request = proto::GetJobInfoRequest { job_id };
    let res = app.get_job_info(request).await.unwrap();
    let res = res.get_ref();
    let job: melon_common::Job = res.into();

    assert_eq!(job.status, JobStatus::Pending);

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_mshow_running() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let job_assignment = mock_setup.job_assignment_receiver.recv().await.unwrap();
    let job_id = job_assignment.job_id;

    // should be marked as running now
    let request = proto::GetJobInfoRequest { job_id };
    let res = app.get_job_info(request).await.unwrap();
    let res = res.get_ref();
    let job: melon_common::Job = res.into();

    assert_eq!(job.status, JobStatus::Running);

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_mshow_failed() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let job_assignment = mock_setup.job_assignment_receiver.recv().await.unwrap();
    let job_id = job_assignment.job_id;
    let job_result = proto::JobResult {
        job_id: job_assignment.job_id,
        status: proto::JobStatus::Failed.into(),
    };
    let _ = app.submit_job_result(job_result).await.unwrap();

    // should be marked as failed now
    let request = proto::GetJobInfoRequest { job_id };
    let res = app.get_job_info(request).await.unwrap();
    let res = res.get_ref();
    let job: melon_common::Job = res.into();

    assert_eq!(job.status, JobStatus::Failed);

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_mshow_completed() {
    let app = spawn_app().await;
    let mut mock_setup = setup_mock_worker().await;
    let info = get_node_info(mock_setup.port);
    app.register_node(info).await.unwrap();
    let submission = get_job_submission();
    let _ = app.submit_job(submission.clone()).await.unwrap();
    let job_assignment = mock_setup.job_assignment_receiver.recv().await.unwrap();
    let job_id = job_assignment.job_id;
    let job_result = proto::JobResult {
        job_id: job_assignment.job_id,
        status: proto::JobStatus::Completed.into(),
    };
    let _ = app.submit_job_result(job_result).await.unwrap();

    // should be marked as completed now
    let request = proto::GetJobInfoRequest { job_id };
    let res = app.get_job_info(request).await.unwrap();
    let res = res.get_ref();
    let job: melon_common::Job = res.into();

    assert_eq!(job.status, JobStatus::Completed);

    mock_setup.server_notifier.send(()).unwrap();
    mock_setup.server_handle.await.unwrap();
}

#[tokio::test]
async fn test_mshow_unknown_id() {
    let app = spawn_app().await;

    // should be marked as completed now
    let request = proto::GetJobInfoRequest { job_id: 10 };
    let res = app.get_job_info(request).await;

    assert!(res.is_err());
    if let Err(e) = res {
        if let Some(status) = e.downcast_ref::<Status>() {
            assert_eq!(status.code(), tonic::Code::NotFound);
            assert_eq!(status.message(), "Job ID not found 10");
        } else {
            panic!("Error is not a tonic::Status: {:?}", e);
        }
    }
}
