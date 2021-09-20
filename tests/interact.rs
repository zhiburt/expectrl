#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[ignore = "It requires manual interaction; Or it's necessary to redirect an stdin of current process"]
#[test]
fn interact_callback() {
    let mut session = expectrl::spawn("cat").unwrap();

    let opts = expectrl::interact::InteractOptions::default().on_input("123", |session| {
        session.send_line("Hello World")?;
        Ok(())
    });

    opts.interact(&mut session).unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[ignore = "It requires manual interaction; Or it's necessary to redirect an stdin of current process"]
#[test]
fn interact_callback() {
    let session = expectrl::spawn("cat").unwrap();

    let session = std::sync::Arc::new(async_lock::Mutex::new(session));
    let session_clone = session.clone();

    let opts = expectrl::interact::InteractOptions::default().on_input("123", move || {
        let session_clone = session_clone.clone();
        async move {
            session_clone.await.send_line("Hello World").await?;
            Ok(())
        }
    });

    futures_lite::future::block_on(async {
        let mut s = session.lock().await;
        opts.interact(&mut s).await
    })
    .unwrap();
}
