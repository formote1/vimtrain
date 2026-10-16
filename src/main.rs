//! vimtrain — train vim motions in the terminal.
//! Keys taught follow https://vim.rtorr.com/ (movement sections in v1).

mod app;
mod coach;
mod engine;
mod game;
mod settings;
mod term;
mod theme;
mod ui;

use std::io::{self, Stdout};

use crossterm::event::{self, Event, KeyEventKind};

use app::App;

const HELP: &str = "\
vimtrain — train vim motions in your terminal

USAGE:
    vimtrain [--skill N]

OPTIONS:
    --skill N    jump straight to skill 1-25 intro (if unlocked)
    -h, --help   this text
    --version    version

GAME:
    Collect vim motions one skill at a time (keys: vim.rtorr.com).
    Move the cursor onto each green goal. Fewest keys vs par wins.
    Master each skill with 2 wins to unlock the next.
    Enter next skill, r replay, ? help, F2 settings, F3 hint,
    : command bar, Esc menu, Ctrl+C quit.
";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("vimtrain {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let mut app = App::new();
    if let Some(i) = args.iter().position(|a| a == "--skill") {
        match args.get(i + 1).and_then(|s| s.parse::<usize>().ok()) {
            Some(n) if (1..=game::curriculum::SKILL_COUNT).contains(&n) => {
                app.start_skill(n - 1);
            }
            _ => {
                eprintln!("--skill needs 1-{}", game::curriculum::SKILL_COUNT);
                std::process::exit(2);
            }
        }
    }

    let mut out = match term::Term::init() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Could not take over terminal: {e}");
            std::process::exit(1);
        }
    };
    let _guard = term::TermGuard;
    let res = run(&mut out, &mut app);
    term::Term::restore(&mut out);
    if let Err(e) = res {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    println!("Thanks for training! Total score: {}", app.total_score);
}

fn run(out: &mut Stdout, app: &mut App) -> io::Result<()> {
    ui::render(out, app)?;
    loop {
        if app.should_quit {
            break;
        }
        match event::read()? {
            Event::Key(k) => {
                if k.kind == KeyEventKind::Release {
                    continue;
                }
                app.handle_key(k.code, k.modifiers);
                if app.should_quit {
                    break;
                }
                ui::render(out, app)?;
            }
            Event::Mouse(m) => {
                if app.handle_mouse(m) {
                    if app.should_quit {
                        break;
                    }
                    ui::render(out, app)?;
                }
            }
            Event::Resize(_, _) => ui::render(out, app)?,
            _ => {}
        }
    }
    Ok(())
}
