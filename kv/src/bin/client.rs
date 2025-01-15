use clap::Parser;
extern crate termion;
use kv::storage::util::validate;
use std::{
    error::Error,
    io::{stdin, stdout, Write},
    process::exit,
};
use termion::{color, input::TermRead};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

type AnyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    addr: String,
}

fn handle_resp(resp: Vec<u8>) -> AnyResult<()> {
    let str_resp = String::from_utf8(resp)?.trim().to_owned();

    let mut stdout_lock = stdout().lock();
    if str_resp == "undefined" {
        write!(stdout_lock, "{}", color::Fg(color::AnsiValue(242)))?;
    } else if str_resp.contains("ERR") || str_resp.contains("WARN") {
        write!(stdout_lock, "{}", color::Fg(color::Red))?;
    } else if str_resp.contains("OK") {
        write!(stdout_lock, "{}", color::Fg(color::Green))?;
    }
    println!("{}", str_resp);
    write!(stdout_lock, "{}", color::Fg(color::Reset))?;

    Ok(())
}

struct History {
    cmds: Vec<String>,
    pos: usize,
}

#[allow(unused)]
impl History {
    fn init() -> History {
        History {
            cmds: Vec::new(),
            pos: 0,
        }
    }

    fn append(&mut self, cmd: String) {
        self.cmds.push(cmd);
        self.pos += 1;
    }

    fn fetch_update(&mut self, dir: u8, mut buf: String) {
        if self.pos == 1 && dir == 1 {
            return;
        }

        if dir == 0 {
            self.pos -= 1;
        } else if dir == 1 && self.pos != self.cmds.len() {
            self.pos += 1;
        }
        buf = self.cmds[self.pos - 1].clone()
    }

    fn pos_end(&mut self) {
        self.pos = self.cmds.len();
    }
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    let args = Args::parse();

    let res = TcpStream::connect(args.addr).await;
    // let mut stdout = stdout().into_raw_mode()?;
    let mut stdout = stdout().lock();

    // let mut history = History::init();

    if res.is_err() {
        write!(stdout, "{}", color::Fg(color::Red))?;
        // stdout.suspend_raw_mode()?;
        println!("connection refused.");
        write!(stdout, "{}", color::Fg(color::Reset))?;
        stdout.flush()?;
        exit(1);
    }

    // let pos = stdout.cursor_pos().unwrap();
    // write!(stdout, "{}", termion::cursor::Goto(1, pos.1))?;

    let mut stream = res.unwrap();
    let mut resp_buf = Vec::new();
    stream.read_buf(&mut resp_buf).await?;
    handle_resp(resp_buf)?;

    loop {
        // stdout.activate_raw_mode()?;
        // let pos = stdout.cursor_pos().unwrap();
        // write!(stdout, "{}", termion::cursor::Goto(1, pos.1))?;
        stdout.flush()?;
        stdout.write_all(b"> ")?;
        stdout.flush()?;

        // let mut buf = String::new();
        let mut stdin = stdin().lock();
        let buf = stdin.read_line()?.unwrap();

        // for k in stdin.keys() {
        //     match k.as_ref().unwrap() {
        //         Key::Char(c) => {
        //             if c == &'\n' {
        //                 stdout.write_all(b"\n")?;
        //                 stdout.flush()?;
        //                 let pos = stdout.cursor_pos().unwrap();
        //                 write!(stdout, "{}", termion::cursor::Goto(1, pos.1))?;
        //                 stdout.flush()?;
        //                 break;
        //             }
        //             buf.push(c.to_owned());
        //             stdout.write_all(format!("{}", c).as_bytes())?;
        //             stdout.flush()?;
        //         }
        //         Key::Left => {
        //             let pos = stdout.cursor_pos().unwrap();
        //             if pos.0 > 2 {
        //                 write!(stdout, "{}", termion::cursor::Left(1))?;
        //                 stdout.flush()?;
        //             }
        //         }
        //         // TODO: unimplemented
        //         // Key::Up => {}
        //         // Key::Down => {}
        //         Key::Backspace => {
        //             write!(
        //                 stdout,
        //                 "{}{}",
        //                 termion::cursor::Left(1),
        //                 termion::clear::AfterCursor
        //             )?;
        //             stdout.flush()?;
        //             buf.pop();
        //         }
        //         Key::Ctrl('x') => {
        //             stdout.suspend_raw_mode()?;
        //             let _ = stream.shutdown().await;
        //             exit(1);
        //         }
        //         _ => {}
        //     }
        // }

        // stdout.suspend_raw_mode()?;

        if buf == "exit" {
            let _ = stream.shutdown().await;
            exit(1);
        }

        // history.append(buf.clone());
        // history.pos_end();

        if validate(buf.clone()).is_some() {
            stream.write_all(&buf.into_bytes()).await?;

            let mut resp_buf = Vec::new();
            stream.read_buf(&mut resp_buf).await?;
            handle_resp(resp_buf.clone())?;
        } else {
            write!(stdout, "{}", color::Fg(color::Red))?;
            println!("invalid command.");
            write!(stdout, "{}", color::Fg(color::Reset))?;
        }
    }
}
