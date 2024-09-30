pub mod app;
pub mod console;
mod parallely;

use crate::app::{App, ShutdownReason};
use crate::parallely::Parallely;
use clap::Parser;
use color_eyre::Help;
use ratatui::crossterm::ExecutableCommand;
use std::process::exit;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // self init
    let _guard = try_init()?;

    let parallely = match Parallely::try_parse() {
        Ok(p) => p,
        Err(e) => {
            restore();
            eprintln!("{}", e);
            exit(1);
        }
    };
    tracing::info!("{:#?}", parallely);

    // ratatui init
    let mut terminal = ratatui::try_init()?;
    terminal.clear()?;

    let mut app = App::default();
    let reason = app.run(terminal, parallely).await?;
    tracing::info!("shutdown with: {:?}", reason);

    // ratatui restore
    ratatui::try_restore()
        .with_suggestion(|| "Failed to restore terminal. Run [reset] to recover")?;

    // self restore
    try_restore()?;

    if let ShutdownReason::End(results) = reason {
        results.into_iter().for_each(|result| match result {
            Ok(result) => println!("{}", result),
            Err(error) => eprintln!("{}", error),
        });
    }

    Ok(())
}

fn try_init() -> color_eyre::Result<WorkerGuard> {
    color_eyre::install()?;

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));

    let filter = EnvFilter::new("info").add_directive("parallely=debug".parse()?);

    let current_dir = std::env::current_dir()?;
    let file_appender = tracing_appender::rolling::daily(current_dir.join("logs"), "parallely");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer().with_writer(non_blocking);

    // let stdout_layer = tracing_subscriber::fmt::layer().pretty();

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        // .with(stdout_layer)
        .init();

    std::io::stdout().execute(crossterm::event::EnableMouseCapture)?;
    std::io::stdout().execute(crossterm::event::EnableFocusChange)?;

    Ok(guard)
}

fn restore() {
    if let Err(err) = try_restore() {
        eprintln!("Failed to restore terminal: {:#?}", err);
        eprintln!("Run [reset] to recover");
    }
}

fn try_restore() -> color_eyre::Result<()> {
    std::io::stdout().execute(crossterm::event::DisableMouseCapture)?;
    std::io::stdout().execute(crossterm::event::DisableFocusChange)?;
    Ok(())
}
