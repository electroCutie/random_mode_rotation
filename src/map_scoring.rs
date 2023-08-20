use std::rc::Rc;

use crate::map_data::{Map, Mode};

static MAX_AGE: u16 = 200;
static ROUND_PENALTY: f64 = 1000.0; // used during inversion of the score
static ROUND_DISCOUNT: f64 = 1.0 / 1.010_889_286_051_700_5; // ~ 64th root of 2, so the penalty halves every 64 rounds
static CROSS_TYPE_ROUND_DISCOUNT: f64 = 1.0 / 1.059_463_094_359_295_3; // ~ 12th root of 2, so the penalty halves every 12 rounds

static PENALTY_NONLINEARITY: f64 = 1.4; // penalty raised to this power before inverting
static AGE_POW: f64 = 0.6; // age raised to this power before being multiplied by the inverted penalty

pub struct MapScoring {
    pub map: Rc<Map>,
    pub age: u16,
    pub cross_type_sibling_penalty: f64,
    pub penalty: f64,
}

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
                self.cross_type_sibling_penalty +=
                    self.map.mode.mode_discount(other_map.mode) * ROUND_PENALTY;
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

        assert!(!s.is_nan(), "Score was NaN, this should not be possible");

        (s, self.map)
    }
}

fn normalize_scores(scores: &[(f64, Rc<Map>)]) -> Vec<(f64, Rc<Map>)> {
    let sum: f64 = scores.iter().map(|s| s.0).sum();
    scores.iter().map(|(s, m)| (s / sum, m.clone())).collect()
}

fn get_appropriate_maps(mode: Mode, players: u16, all_maps: &[Rc<Map>]) -> Vec<MapScoring> {
    all_maps
        .iter()
        // only choose maps that are the correct mode and have enough player capacity
        .filter(|m| m.mode == mode && m.players >= players)
        .map(|map| MapScoring {
            map: map.clone(),
            age: MAX_AGE,
            cross_type_sibling_penalty: 1.0,
            penalty: 1.0,
        })
        .collect()
}

pub fn build_scores(
    log: &[Rc<Map>],
    mode: Mode,
    players: u16,
    all_maps: &[Rc<Map>],
) -> Vec<(f64, Rc<Map>)> {
    let mut scores = get_appropriate_maps(mode, players, all_maps);

    // let every valid map see the log to accunulate penalties and age
    for s in &mut scores {
        for l in log {
            s.map_played(l);
        }
    }

    #[cfg(feature = "debug_raw_scores")]
    {
        for s in &scores {
            println!("{} {}", s.penalty, s.map.map_info());
        }
    }

    // turn the map scores into usable numeric scores
    let scores: Vec<(f64, Rc<Map>)> = scores.into_iter().map(MapScoring::final_score).collect();

    // normalize the scores so that all the scores add up to 1 (so we can show the user a %)
    let mut scores = normalize_scores(&scores);

    // Sort the scored maps so that the highest scoring ones come first
    scores.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().reverse());

    #[cfg(feature = "debug_scores")]
    {
        println!();
        for (s, m) in &scores {
            println!("{} {}", s, m.map_info());
        }
    }

    scores
}
