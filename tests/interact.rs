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
