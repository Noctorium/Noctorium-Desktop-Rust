//! One Noctorium at a time: opening it again brings the window already open to the front, rather than a
//! second player fighting the first for the same queue and the same sound.
//!
//! The first to start listens on a port of the computer's own loopback; a second finds it there, asks it to
//! show itself, and leaves. The two greet each other by name, so an unrelated program that happens to hold
//! the port is not mistaken for Noctorium.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

const PORT: u16 = 47_831;
const ASK: &str = "noctorium-rust show";
const ANSWER: &str = "noctorium-rust here";

pub enum Instance {
    /// This is the only one; the listener hears from later ones.
    First(Option<TcpListener>),
    /// Another is already open and has been asked to show itself.
    Second,
}

pub fn claim() -> Instance {
    if std::env::var_os("NOCTORIUM_ALLOW_SECOND").is_some() {
        return Instance::First(None);
    }
    match TcpListener::bind(("127.0.0.1", PORT)) {
        Ok(listener) => Instance::First(Some(listener)),
        Err(_) if ask_the_other_to_show() => Instance::Second,
        // The port is someone else's: carry on without being the only one.
        Err(_) => Instance::First(None),
    }
}

fn ask_the_other_to_show() -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&([127, 0, 0, 1], PORT).into(), Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    if writeln!(stream, "{ASK}").is_err() {
        return false;
    }
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).is_ok() && answer.trim() == ANSWER
}

/// Hears later starts, and calls `show` for each, on a thread of its own.
pub fn listen(listener: TcpListener, show: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("single-instance".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() && line.trim() == ASK {
                    let _ = writeln!(&stream, "{ANSWER}");
                    show();
                }
            }
        })
        .ok();
}
