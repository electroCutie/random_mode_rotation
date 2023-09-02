use std::{
    cmp::Ordering,
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    io::Write,
    rc::Rc,
};

mod coloring;

mod map_data;
use ansi_term::{ANSIString, Color, Style};
use coloring::MaybeColor;
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
    Percents,
    AllMaps,
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

fn choice<'a, S>(choice: S) -> ANSIString<'a>
where
    S: ToString,
{
    Style::new()
        .fg(Color::White)
        .bold()
        .maybe_color()
        .paint(choice.to_string())
}

fn print_map_choices(
    mode: Mode,
    players: u16,
    random_maps: &[(f64, RcMap)],
) -> Result<(), Box<dyn Error>> {
    let spaces: usize = usize::from(random_maps.len() > 9) + 1;

    let print_map_choice = |idx: usize, random_maps: &[(f64, RcMap)]| {
        let (percent, map) = &random_maps[idx];
        println!(
            " ({}) {} ({}) {}",
            choice(format!("{: >1$}", idx + 1, spaces)),
            map.nickname,
            map.players,
            Style::new()
                .italic()
                .maybe_color()
                .paint(format!("{:.2}%", percent * 100.))
        );
    };

    println!();
    println!("Mode {} for {} players", mode, players);
    for i in 0..random_maps.len() {
        print_map_choice(i, random_maps);
    }
    let space = " ".repeat(spaces - 1);
    println!(" ({}{}) Change Mode", space, choice('m'));
    println!(" ({}{}) Set Players", space, choice('p'));
    println!(" ({}{}) Show Map Percents", space, choice('%'));
    println!(" ({}{}) Choose From All Maps", space, choice('a'));
    println!(" ({}{}) Shuffle", space, choice('s'));
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

fn get_mode_action(map_ct: usize) -> Result<ModeAction, Box<dyn Error>> {
    read_until_valid(|response| {
        let numeric = response.parse::<usize>();
        if let Ok(n) = numeric {
            if n <= map_ct && n > 0 {
                Ok(ModeAction::SelectMap(n - 1))
            } else {
                let err: String = format!("map selection {} out of range 1..{}", n, map_ct);
                Err(err)
            }
        } else {
            match response.as_str() {
                "m" => Ok(ModeAction::ChangeMode),
                "p" => Ok(ModeAction::SetPlayerCt),
                "%" => Ok(ModeAction::Percents),
                "a" => Ok(ModeAction::AllMaps),
                "s" => Ok(ModeAction::Shuffle),
                _ => Err("bad response".into()),
            }
        }
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
        println!(" ({}) {}", choice(idx), mode);
    }
    println!(" ({}) Cancel", choice('c'));
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

fn sort_score<T>(a: &(f64, T), b: &(f64, T)) -> Ordering {
    a.0.partial_cmp(&b.0).unwrap().reverse()
}

fn pick_random_maps(
    log: &[RcMap],
    mode: Mode,
    players: u16,
    all_maps: &[RcMap],
    quiet: bool,
) -> Result<Vec<(f64, RcMap)>, Box<dyn Error>> {
    if !quiet {
        print_flush!("Selecting Options");
    }

    let mut scores = build_scores(log, mode, players, all_maps);
    assert!(!scores.is_empty());

    if !quiet {
        print_flush!(".");
    }

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
                if !quiet {
                    print_flush!(".");
                }
                break;
            }
        }

        if random_maps.len() >= 3 {
            break;
        }
    }

    random_maps.sort_unstable_by(sort_score);

    Ok(random_maps)
}

fn print_all_maps_for_mode(log: &[RcMap], all_maps: &[RcMap]) -> Result<(), Box<dyn Error>> {
    if let Some(mode) = prompt_for_mode()? {
        let mut scores = build_scores(log, mode, 0, all_maps);
        assert!(!scores.is_empty());
        scores.sort_unstable_by(sort_score);

        println!();
        println!("All maps for {}", mode);
        for (score, map) in scores {
            println!(
                "  {} ({}) {}",
                map.nickname,
                map.players,
                Style::new()
                    .italic()
                    .maybe_color()
                    .paint(format!("{:.2}%", score * 100.))
            );
        }
        println!();
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let (groups, maps) = load_map_data()?;

    let all_maps: Vec<RcMap> = maps.values().map(Rc::clone).collect();

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).filter(|a| *a == "--simulate").is_some() {
        let groups: Vec<RcGroup> = groups.values().map(Rc::clone).collect();
        simulate(&groups, &all_maps)?;
        return Ok(());
    }

    println!("Loaded {} maps", maps.len());

    let mut log = load_log(&maps)?;
    println!("Loaded Log with {} entries", log.len());

    // Initial state
    let mut mode = match log.last() {
        None => Mode::TD,
        Some(m) => m.mode.next(),
    };
    let mut players = 16u16;

    let mut show_all_maps = false;
    // main loop
    loop {
        let random_maps = if show_all_maps {
            show_all_maps = false;
            build_scores(&log, mode, players, &all_maps)
        } else {
            pick_random_maps(&log, mode, players, &all_maps, false)?
        };
        print_map_choices(mode, players, &random_maps)?;

        match get_mode_action(random_maps.len())? {
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
            ModeAction::Percents => print_all_maps_for_mode(&log, &all_maps)?,
            ModeAction::AllMaps => show_all_maps = true,
            ModeAction::Shuffle => {} // No action required, just loop
        }
    }
}

fn simulate(all_groups: &[RcGroup], all_maps: &[RcMap]) -> Result<(), Box<dyn Error>> {
    let mut log = Vec::new();
    let mut mode = Mode::TD;

    for _ in 0..10_000 {
        let random_maps = pick_random_maps(&log, mode, 16, all_maps, true)?;
        let map = &random_maps.get(0).unwrap().1;

        log.push(map.clone());
        mode = mode.next();
    }

    let mut counts: HashMap<u16, u32> = HashMap::new();

    for m in log {
        let e = counts.entry(m.id);
        match e {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let v = e.get_mut();
                *v += 1;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(1);
            }
        }
    }

    for mode in Mode::ordered() {
        for group in all_groups {
            for map in &group.variants {
                if map.mode != mode {
                    continue;
                }

                let ct = counts.get(&map.id);
                if let Some(ct) = ct {
                    println!("\"{}\",\"{}\",{}", mode, map.nickname, ct);
                }
            }
        }
    }

    Ok(())
}
