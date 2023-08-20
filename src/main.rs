use std::{error::Error, io::Write, rc::Rc};

mod map_data;
use map_data::*;

mod play_log;
use play_log::*;
use rand::random;

struct MapScoring {
    map: Rc<Map>,
    age: u16,
    cross_type_sibling_penalty: f64,
    penalty: f64,
}

static MAX_AGE: u16 = 200;
static ROUND_PENALTY: f64 = 1000.0;
static ROUND_DISCOUNT: f64 = 1.0 / 1.010_889_286_051_700_5; // ~ 64th root of 2, so the penalty halves every 64 rounds
static CROSS_TYPE_ROUND_DISCOUNT: f64 = 1.0 / 1.059_463_094_359_295_3; // ~ 12th root of 2, so the penalty halves every 12 rounds

static PENALTY_NONLINEARITY: f64 = 1.4; // penalty raised to this power before inverting
static AGE_POW: f64 = 0.6; // age raised to this power before being multiplied by the inverted penalty

impl MapScoring {
    fn map_played(&mut self, other_map: &Map) {
        self.penalty *= ROUND_DISCOUNT;
        self.cross_type_sibling_penalty *= CROSS_TYPE_ROUND_DISCOUNT;
        self.age = MAX_AGE.min(self.age + 1);

        if *other_map == *self.map {
            self.age = 1;
        }

        let my_g = self.map.group();
        let other_g = other_map.group();

        if my_g == other_g {
            if self.map.mode == other_map.mode {
                self.penalty += ROUND_PENALTY;
            } else {
                // we are in a group with the other map, apply a recent-ness penalty, discounted by type
                self.penalty += self.map.mode.mode_discount(other_map.mode) * ROUND_PENALTY;
            }
        }
    }

    fn final_score(self) -> (f64, Rc<Map>) {
        // penalty is the sum of both types
        let s = self.penalty + self.cross_type_sibling_penalty;
        // make the penalty non-linear to further penalize recent plays & invert
        let s = 1000. / s.powf(PENALTY_NONLINEARITY);
        // raise the chance of maps that haven't been played in a while
        let s = s * (self.age as f64).powf(AGE_POW);
        // don't let the values go TOO sideways
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
            age: MAX_AGE,
            cross_type_sibling_penalty: 1.0,
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
    for (s, m) in &scores {
        println!("{} {}", s, m.map_info());
    }

    scores
}

fn main() -> Result<(), Box<dyn Error>> {
    let (_, maps) = load_map_data()?;
    println!("Loaded {} maps", maps.len());

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
        std::io::stdout().flush()?;

        let mut scores = build_scores(&log, mode, players, map_list.as_slice());
        assert!(!scores.is_empty());

        print!(".");
        std::io::stdout().flush()?;

        let mut random_maps: Vec<(f64, Rc<Map>)> = Vec::new();

        loop {
            let sum: f64 = scores.iter().map(|s| s.0).sum();
            let mut random: f64 = random::<f64>() * sum;
            for ((s, m), idx) in scores.iter().zip(0..) {
                random -= *s;
                if random <= 0. {
                    assert!(!random_maps.iter().any(|(_, e)| m.id == e.id));
                    random_maps.push((*s, m.clone()));
                    scores.remove(idx);
                    print!(".");
                    std::io::stdout().flush()?;
                    break;
                }
            }

            if random_maps.len() >= 3 {
                break;
            }
        }

        random_maps.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap().reverse());

        println!();

        println!("Mode {} ({})", mode, players);
        println!(
            " (1) {} ({:.2}%)",
            random_maps[0].1.map_info(),
            random_maps[0].0 * 100.
        );
        println!(
            " (2) {} ({:.2}%)",
            random_maps[1].1.map_info(),
            random_maps[1].0 * 100.
        );
        println!(
            " (3) {} ({:.2}%)",
            random_maps[2].1.map_info(),
            random_maps[2].0 * 100.
        );
        println!(" (m) Change Mode");
        println!(" (p) Set Players");
        println!(" (s) Shuffle");
        print!("> ");
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
                    print!("bad response\n> ");
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
                println!("Select Mode:");
                println!(" (1) TD");
                println!(" (2) DM");
                println!(" (3) Chaser");
                println!(" (4) BR");
                println!(" (5) Captain");
                println!(" (6) Siege");
                println!(" (c) Cancel");
                print!("> ");
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
                        Some('c') => {}
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
                print!("How many players?\n> ");
                std::io::stdout().flush()?;
                loop {
                    let response = read_line::read_line();
                    let response = response.trim();
                    let p = response.parse::<u16>();
                    match p {
                        Ok(n @ 8..=16) => players = n,
                        _ => {
                            println!("invalid number, expecting 8-16\n> ");
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
}
