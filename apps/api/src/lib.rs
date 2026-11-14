    if let Err(error) = runner::execute(
        &request,
        std::time::Duration::from_secs(timeout_seconds as u64),
        cancel_receiver,
    )
    .await
    {
        state
            .cancellations
            .lock()
            .expect("cancellation registry is not poisoned")
            .remove(&run_id);
        if matches!(&error, runner::RunnerError::Canceled) {
            repository.cancel_run(run_id).await?;
        } else {
            repository.fail_run(run_id, &error.to_string()).await?;
        }
        return Err(ApiError::Execution(error.to_string()));
    }
    state
        .cancellations
