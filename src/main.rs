use std::{error::Error, io::Write, rc::Rc};

mod map_data;
use croot::roots::principal_root;
use map_data::*;

mod play_log;
use play_log::*;
use rand::random;

struct MapScoring {
    map: Rc<Map>,
    penalty: f64,
}

static ROUND_PENALTY: f64 = 1000.0;
static ROUND_DISCOUNT: f64 = 1.0 / 1.010_889_286_051_700_5; // ~ 64th root of 2, so the penalty halves every 64 rounds

impl MapScoring {
    fn map_played(&mut self, other_map: &Map) {
        // TODO: maybe someday this can be a Lazy Cell
        self.penalty *= ROUND_DISCOUNT;

        let my_gid = self.map.group().gid;
        let other_gid = other_map.group().gid;

        if my_gid == other_gid {
            // we are in a group with the other map, apply a recent-ness penalty, discounted by type
            self.penalty += self.map.mode.mode_discount(other_map.mode) * ROUND_PENALTY;
        }
    }

    fn final_score(self) -> (f64, Rc<Map>) {
        let s = 1000. / self.penalty.powf(1.4);
        let s = s.clamp(0.001, 100000.);
        (s, self.map)
    }
}

fn normalize_scores(scores: &[(f64, Rc<Map>)]) -> Vec<(f64, Rc<Map>)> {
    let sum: f64 = scores.iter().map(|s| s.0).sum();
    scores.iter().map(|(s, m)| (s / sum, m.clone())).collect()
}

fn build_scores(
    log: &[Rc<Map>],
    mode: Mode,
    players: u16,
    all_maps: &[Rc<Map>],
) -> Vec<(f64, Rc<Map>)> {
    let mut scores: Vec<MapScoring> = all_maps
        .iter()
        .filter(|m| m.mode == mode && m.players >= players)
        .map(|map| MapScoring {
            map: map.clone(),
            penalty: 1.0,
        })
        .collect();

    for s in &mut scores {
        for l in log {
            s.map_played(l);
        }
    }

    // for s in &scores {
    //     println!("{} {}", s.penalty, s.map.map_info());
    // }

    let scores: Vec<(f64, Rc<Map>)> = scores.into_iter().map(MapScoring::final_score).collect();
    let mut scores = normalize_scores(scores.as_slice());
    scores.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().reverse());

    println!();

    // for (s, m) in &scores {
    //     println!("{} {}", s, m.map_info());
    // }

    scores
}

fn main() -> Result<(), Box<dyn Error>> {
    let (_, maps) = load_map_data()?;
    println!("Loaded {} maps", maps.len());

    // append_log(maps.get(&2).unwrap())?;
    let mut log = load_log(&maps)?;
    println!("Loaded Log with {} entries", log.len());

    let mut mode = match log.last() {
        None => Mode::TD,
        Some(m) => m.mode.next(),
    };

    let mut players = 16u16;

    let map_list: Vec<Rc<Map>> = maps.values().map(Rc::clone).collect();

    loop {
        print!("Selecting Options");
        let scores = build_scores(&log, mode, players, map_list.as_slice());
        assert!(!scores.is_empty());
        print!(".");

        let mut random_maps: Vec<(f64, Rc<Map>)> = Vec::new();

        loop {
            let mut random: f64 = random();
            for (s, m) in &scores {
                random -= *s;
                if random <= 0. {
                    //TODO: reject maps we already have selected
                    if !random_maps.iter().any(|(_, e)| m.id == e.id) {
                        random_maps.push((*s, m.clone()));
                        break;
                    }
                }
            }
            print!(".");

            if random_maps.len() >= 3 {
                break;
            }
        }
        println!();

        print!("Mode {} ({})\n Maps:\n (1) {} ({:.2}%)\n (2) {} ({:.2}%)\n (3) {} ({:.2}%)\n (m) Change Mode\n (p) Set Players\n (s) Shuffle\n > ",
            mode, players,
            random_maps[0].1.map_info(), random_maps[0].0 * 100.,
            random_maps[1].1.map_info(), random_maps[1].0 * 100.,
            random_maps[2].1.map_info(), random_maps[2].0 * 100.,
        );
        std::io::stdout().flush()?;

        let selection = loop {
            let response = read_line::read_line();
            let response = response.chars().next();
            match response {
                Some('1') => break 1,
                Some('2') => break 2,
                Some('3') => break 3,
                Some('m') => break 4,
                Some('p') => break 5,
                Some('s') => break 6,
                _ => {
                    print!("bad response\n > ");
                    std::io::stdout().flush()?;
                }
            }
        };

        match selection {
            s @ 1..=3 => {
                let map = random_maps.get(s - 1).unwrap().1.clone();
                append_log(map.as_ref())?;
                log.push(map.clone());
                mode = mode.next();
                println!("{} Selected. Have Fun!\n", map.map_info());
            }
            4 => {
                print!( "Select Mode:\n (1) TD\n (2) DM\n (3) Chaser\n (4) BR\n (5) Captain (6) Siege\n > ");
                std::io::stdout().flush()?;
                loop {
                    let response = read_line::read_line();
                    let response = response.chars().next();
                    match response {
                        Some('1') => mode = Mode::TD,
                        Some('2') => mode = Mode::DM,
                        Some('3') => mode = Mode::Chaser,
                        Some('4') => mode = Mode::BR,
                        Some('5') => mode = Mode::Captain,
                        Some('6') => mode = Mode::Siege,
                        _ => {
                            print!("bad response\n > ");
                            std::io::stdout().flush()?;
                            continue;
                        }
                    }
                    break;
                }
            }
            5 => {
                print!("How many players?\n > ");
                std::io::stdout().flush()?;
                loop {
                    let response = read_line::read_line();
                    let response = response.trim();
                    let p = response.parse::<u16>();
                    match p {
                        Ok(n @ 8..=16) => players = n,
                        _ => {
                            println!("invalid number, expecting 8-16\n > ");
                            continue;
                        }
                    }
                    break;
                }
            }
            6 => println!("Shuffling"),
            _ => {
                println!("This should be impossible. Shuffling")
            }
        }
    }

    // let round_discount:f64 = principal_root(2., 64.).re;
    // println!("{} {}", ROUND_DISCOUNT, ROUND_DISCOUNT.powi(64));
}
