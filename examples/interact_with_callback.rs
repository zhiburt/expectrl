use expectrl::{interact::actions::lookup::Lookup, spawn, stream::stdin::Stdin, Regex};

#[derive(Debug, Default)]
struct State {
    stutus_verification_counter: Option<usize>,
    wait_for_continue: Option<bool>,
    pressed_yes_on_continue: Option<bool>,
}

#[cfg(not(all(windows, feature = "polling")))]
#[cfg(not(feature = "async"))]
#[cfg(unix)]
fn main() {
    let mut output_action = Lookup::new();
    let mut input_action = Lookup::new();
    let mut state = State::default();

    let mut session = spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let mut stdin = Stdin::open().unwrap();
    let stdout = std::io::stdout();

    let (is_alive, status) = {
        let mut interact = session.interact(&mut stdin, stdout).with_state(&mut state);
        interact
            .set_output_action(move |ctx| {
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

                Ok(false)
            })
            .set_input_action(move |ctx| {
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

                Ok(false)
            });

        let is_alive = interact.spawn().expect("Failed to start interact");

        (is_alive, interact.get_status())
    };

    if !is_alive {
        println!("The process was exited");
        #[cfg(unix)]
        println!("Status={:?}", status);
    }

    stdin.close().unwrap();

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

#[cfg(all(unix, feature = "async"))]
fn main() {
    // let mut output_action = Lookup::new();
    // let mut input_action = Lookup::new();
    // let mut state = State::default();

    // let mut session = spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    // let mut stdin = Stdin::open().unwrap();
    // let stdout = std::io::stdout();

    // let mut interact = session.interact(&mut stdin, stdout).with_state(&mut state);
    // interact
    //     .set_output_action(|mut ctx| {
    //         let m = output_action.on(ctx.buf, ctx.eof, "Continue [y/n]:")?;
    //         if m.is_some() {
    //             ctx.state.wait_for_continue = Some(true);
    //         };

    //         let m = output_action.on(ctx.buf, ctx.eof, Regex("status:\\s*.*\\w+.*\\r\\n"))?;
    //         if m.is_some() {
    //             ctx.state.stutus_verification_counter =
    //                 Some(ctx.state.stutus_verification_counter.map_or(1, |c| c + 1));
    //             output_action.clear();
    //         }

    //         Ok(false)
    //     })
    //     .set_input_action(|mut ctx| {
    //         let m = input_action.on(ctx.buf, ctx.eof, "y")?;
    //         if m.is_some() {
    //             if let Some(_a @ true) = ctx.state.wait_for_continue {
    //                 ctx.state.pressed_yes_on_continue = Some(true);
    //             }
    //         };

    //         let m = input_action.on(ctx.buf, ctx.eof, "n")?;
    //         if m.is_some() {
    //             if let Some(_a @ true) = ctx.state.wait_for_continue {
    //                 ctx.state.pressed_yes_on_continue = Some(false);
    //             }
    //         }

    //         Ok(false)
    //     });

    // let is_alive =
    //     futures_lite::future::block_on(interact.spawn()).expect("Failed to start interact");

    // if !is_alive {
    //     println!("The process was exited");
    //     #[cfg(unix)]
    //     println!("Status={:?}", interact.get_status());
    // }

    // stdin.close().unwrap();

    // println!("RESULTS");
    // println!(
    //     "Number of time 'Y' was pressed = {}",
    //     state.pressed_yes_on_continue.unwrap_or_default()
    // );
    // println!(
    //     "Status counter = {}",
    //     state.stutus_verification_counter.unwrap_or_default()
    // );
}

#[cfg(windows)]
fn main() {}
