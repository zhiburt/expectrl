use expectrl::{interact::actions::lookup::Lookup, spawn, stream::stdin::Stdin, Regex};

#[derive(Debug, Default)]
struct State {
    stutus_verification_counter: Option<usize>,
    wait_for_continue: Option<bool>,
    pressed_yes_on_continue: Option<bool>,
}

#[cfg(all(not(feature = "async"), not(all(windows, feature = "polling"))))]
fn main() {
    let mut output_action = Lookup::new();
    let mut input_action = Lookup::new();

    let mut session = spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let mut stdin = Stdin::open().unwrap();
    let stdout = std::io::stdout();

    let (state, is_alive) = session
        .interact(&mut stdin, stdout)
        .set_state(State::default())
        .on_output(|mut ctx| {
            let m = output_action.on(ctx.buf, ctx.eof, "Continue [y/n]:")?;
            if m.is_some() {
                ctx.state.wait_for_continue = Some(true);
            };

            let m = output_action.on(ctx.buf, ctx.eof, Regex("status:\\s*.*\\w+.*\\r\\n"))?;
            if m.is_some() {
                ctx.state.stutus_verification_counter =
                    Some(ctx.state.stutus_verification_counter.map_or(1, |c| c + 1));
                output_action.clear();
            }

            Ok(())
        })
        .on_input(|mut ctx| {
            let m = input_action.on(ctx.buf, ctx.eof, "y")?;
            if m.is_some() {
                if let Some(_a @ true) = ctx.state.wait_for_continue {
                    ctx.state.pressed_yes_on_continue = Some(true);
                }
            };

            let m = input_action.on(ctx.buf, ctx.eof, "n")?;
            if m.is_some() {
                if let Some(_a @ true) = ctx.state.wait_for_continue {
                    ctx.state.pressed_yes_on_continue = Some(false);
                }
            }

            Ok(())
        })
        .spawn()
        .expect("Failed to start interact");

    stdin.close().unwrap();

    if !is_alive {
        println!("The process was exited");
    }

    println!("RESULTS");
    println!(
        "Number of time 'Y' was pressed = {}",
        state.pressed_yes_on_continue.unwrap_or_default()
    );
    println!(
        "Status counter = {}",
        state.stutus_verification_counter.unwrap_or_default()
    );
}

#[cfg(feature = "async")]
fn main() {
    use futures_lite::future::block_on;

    let mut output_action = Lookup::new();
    let mut input_action = Lookup::new();

    let mut session = spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let mut stdin = Stdin::open().unwrap();
    let stdout = std::io::stdout();

    let interact = session
        .interact(&mut stdin, stdout)
        .set_state(State::default())
        .on_output(|mut ctx| {
            let m = output_action.on(ctx.buf, ctx.eof, "Continue [y/n]:")?;
            if m.is_some() {
                ctx.state.wait_for_continue = Some(true);
            };

            let m = output_action.on(ctx.buf, ctx.eof, Regex("status:\\s*.*\\w+.*\\r\\n"))?;
            if m.is_some() {
                ctx.state.stutus_verification_counter =
                    Some(ctx.state.stutus_verification_counter.map_or(1, |c| c + 1));
            }

            Ok(())
        })
        .on_input(|mut ctx| {
            let m = input_action.on(ctx.buf, ctx.eof, "y")?;
            if m.is_some() {
                if let Some(_a @ true) = ctx.state.wait_for_continue {
                    ctx.state.pressed_yes_on_continue = Some(true);
                }
            };

            let m = input_action.on(ctx.buf, ctx.eof, "n")?;
            if m.is_some() {
                if let Some(_a @ true) = ctx.state.wait_for_continue {
                    ctx.state.pressed_yes_on_continue = Some(false);
                }
            }

            Ok(())
        });

    let (state, is_alive) = block_on(interact.spawn()).expect("Failed to start interact");

    stdin.close().unwrap();

    if !is_alive {
        println!("The process was exited")
    }

    println!("RESULTS");
    println!(
        "Number of time 'Y' was pressed = {}",
        state.pressed_yes_on_continue.unwrap_or_default()
    );
    println!(
        "Status counter = {}",
        state.stutus_verification_counter.unwrap_or_default()
    );
}

#[cfg(all(windows, feature = "polling"))]
fn main() {
    let mut output_action = Lookup::new();
    let mut input_action = Lookup::new();

    let mut session = spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let stdin = Stdin::open().unwrap();
    let stdout = std::io::stdout();

    let (state, _) = session
        .interact(stdin, stdout)
        .set_state(State::default())
        .on_output(|mut ctx| {
            let m = output_action.on(ctx.buf, ctx.eof, "Continue [y/n]:")?;
            if m.is_some() {
                ctx.state.wait_for_continue = Some(true);
            };

            let m = output_action.on(ctx.buf, ctx.eof, Regex("status:\\s*.*\\w+.*\\r\\n"))?;
            if m.is_some() {
                ctx.state.stutus_verification_counter =
                    Some(ctx.state.stutus_verification_counter.map_or(1, |c| c + 1));
                output_action.clear();
            }

            Ok(())
        })
        .on_input(|mut ctx| {
            let m = input_action.on(ctx.buf, ctx.eof, "y")?;
            if m.is_some() {
                if let Some(_a @ true) = ctx.state.wait_for_continue {
                    ctx.state.pressed_yes_on_continue = Some(true);
                }
            };

            let m = input_action.on(ctx.buf, ctx.eof, "n")?;
            if m.is_some() {
                if let Some(_a @ true) = ctx.state.wait_for_continue {
                    ctx.state.pressed_yes_on_continue = Some(false);
                }
            }

            Ok(())
        })
        .spawn()
        .expect("Failed to start interact");

    let stdin = Stdin::open().unwrap();
    stdin.close().unwrap();

    println!("RESULTS...........");
    println!(
        "Number of time 'Y' was pressed = {}",
        state.pressed_yes_on_continue.unwrap_or_default()
    );
    println!(
        "Status counter = {}",
        state.stutus_verification_counter.unwrap_or_default()
    );
}
