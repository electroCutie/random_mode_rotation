use std::{
    error::Error,
    fmt::{Debug, Display},
    io::Write,
    rc::Rc,
};

mod map_data;
use map_data::*;

mod play_log;
use play_log::*;
use rand::random;

mod map_scoring;
use map_scoring::*;

enum ModeAction {
    SelectMap(usize),
    ChangeMode,
    SetPlayerCt,
    Shuffle,
}

macro_rules! print_flush {
    ($($pargs:expr),+) => {
        {
            use std::io::stdout;
            print!($($pargs),+);
            stdout().flush()?;
        }
    };
}

fn read_line() -> String {
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Error: unable to read user input");

    input
}

fn print_map_choices(
    mode: Mode,
    players: u16,
    random_maps: &[(f64, RcMap)],
) -> Result<(), Box<dyn Error>> {
    fn print_map_choice(idx: usize, random_maps: &[(f64, RcMap)]) {
        let (percent, map) = &random_maps[idx];
        println!(" ({}) {} ({:.2}%)", idx + 1, map.map_info(), percent * 100.);
    }

    println!();
    println!("Mode {} ({})", mode, players);
    print_map_choice(0, random_maps);
    print_map_choice(1, random_maps);
    print_map_choice(2, random_maps);
    println!(" (m) Change Mode");
    println!(" (p) Set Players");
    println!(" (s) Shuffle");
    print_flush!("> ");

    Ok(())
}

fn read_until_valid<F, T, E>(f: F) -> Result<T, Box<dyn Error>>
where
    F: Fn(String) -> Result<T, E>,
    E: Display + Debug,
{
    loop {
        let response = read_line().trim().to_string();
        let response = f(response);
        match response {
            Ok(v) => break Ok(v),
            Err(err) => print_flush!("{}\n> ", err),
        }
    }
}

fn read_until_valid_char<F, T, E>(f: F) -> Result<T, Box<dyn Error>>
where
    F: Fn(Option<char>) -> Result<T, E>,
    E: Display + Debug,
{
    read_until_valid(|response| f(response.chars().next()))
}

fn get_mode_action() -> Result<ModeAction, Box<dyn Error>> {
    read_until_valid_char(|response| match response {
        Some('1') => Ok(ModeAction::SelectMap(0)),
        Some('2') => Ok(ModeAction::SelectMap(1)),
        Some('3') => Ok(ModeAction::SelectMap(2)),
        Some('m') => Ok(ModeAction::ChangeMode),
        Some('p') => Ok(ModeAction::SetPlayerCt),
        Some('s') => Ok(ModeAction::Shuffle),
        _ => Err("bad response"),
    })
}

fn prompt_for_player_ct() -> Result<u16, Box<dyn Error>> {
    print_flush!("How many players?\n> ");
    read_until_valid(|response| {
        let p = response.parse::<u16>();
        match p {
            Ok(n @ 8..=16) => Ok(n),
            _ => Err("players must be between 8 and 16"),
        }
    })
}

fn prompt_for_mode() -> Result<Option<Mode>, Box<dyn Error>> {
    println!("Select Mode:");
    for (mode, idx) in Mode::ordered().iter().zip(1..) {
        println!(" ({}) {}", idx, mode);
    }
    println!(" (c) Cancel");
    print_flush!("> ");
    read_until_valid_char(|response| match response {
        Some('1') => Ok(Some(Mode::TD)),
        Some('2') => Ok(Some(Mode::DM)),
        Some('3') => Ok(Some(Mode::Chaser)),
        Some('4') => Ok(Some(Mode::BR)),
        Some('5') => Ok(Some(Mode::Captain)),
        Some('6') => Ok(Some(Mode::Siege)),
        Some('c') => Ok(None),
        _ => Err("bad response"),
    })
}

fn pick_random_maps(
    log: &[RcMap],
    mode: Mode,
    players: u16,
    all_maps: &[RcMap],
) -> Result<Vec<(f64, RcMap)>, Box<dyn Error>> {
    print_flush!("Selecting Options");

    let mut scores = build_scores(log, mode, players, all_maps);
    assert!(!scores.is_empty());

    print_flush!(".");

    let mut random_maps: Vec<(f64, RcMap)> = Vec::new();

    loop {
        let sum: f64 = scores.iter().map(|s| s.0).sum();
        let mut random: f64 = random::<f64>() * sum;
        for ((s, m), idx) in scores.iter().zip(0..) {
            random -= *s;
            if random <= 0. {
                assert!(!random_maps.iter().any(|(_, e)| m.id == e.id));
                random_maps.push((*s, m.clone()));
                scores.remove(idx);
                print_flush!(".");
                break;
            }
        }

        if random_maps.len() >= 3 {
            break;
        }
    }

    random_maps.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap().reverse());

    Ok(random_maps)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (_, maps) = load_map_data()?;
    println!("Loaded {} maps", maps.len());

    let mut log = load_log(&maps)?;
    println!("Loaded Log with {} entries", log.len());

    // Initial state
    let mut mode = match log.last() {
        None => Mode::TD,
        Some(m) => m.mode.next(),
    };
    let mut players = 16u16;

    let all_maps: Vec<RcMap> = maps.values().map(Rc::clone).collect();
    let all_maps = all_maps.as_slice();

    // main loop
    loop {
        let random_maps = pick_random_maps(&log, mode, players, all_maps)?;
        print_map_choices(mode, players, &random_maps)?;

        match get_mode_action()? {
            ModeAction::SelectMap(n) => {
                let map = random_maps.get(n).unwrap().1.clone();
                append_log(map.as_ref())?;
                log.push(map.clone());
                mode = mode.next();
                println!("{} Selected. Have Fun!\n", map.map_info());
            }
            ModeAction::ChangeMode => {
                if let Some(m) = prompt_for_mode()? {
                    mode = m;
                }
            }
            ModeAction::SetPlayerCt => players = prompt_for_player_ct()?,
            ModeAction::Shuffle => {} // No action required, just loop
        }
    }
}
