use std::{cell::RefCell, collections::HashMap, error::Error, fmt::Display, fs, rc::Rc};

use ansi_term::{Color, Style};
use json::JsonValue;

use crate::coloring::MaybeColor;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Mode {
    TD,      // lightish blue
    DM,      // red
    Chaser,  // green
    BR,      // purple
    Captain, // pink
    Siege,   // yellow
}

impl Mode {
    pub fn ordered() -> [Self; 6] {
        let mut modes = [
            Mode::TD,
            Mode::DM,
            Mode::Chaser,
            Mode::BR,
            Mode::Captain,
            Mode::Siege,
        ];
        modes.sort_unstable();
        modes
    }

    pub fn console_color(&self) -> Style {
        match self {
            Mode::TD => Style::new().fg(Color::Cyan),
            Mode::DM => Style::new().fg(Color::Red),
            Mode::Chaser => Style::new().fg(Color::Green),
            Mode::BR => Style::new().fg(Color::Purple).dimmed(),
            Mode::Captain => Style::new().fg(Color::Purple),
            Mode::Siege => Style::new().fg(Color::Yellow),
        }
        .bold()
    }

    pub fn next(&self) -> Self {
        match self {
            Mode::TD => Mode::DM,
            Mode::DM => Mode::Chaser,
            Mode::Chaser => Mode::BR,
            Mode::BR => Mode::Captain,
            Mode::Captain => Mode::Siege,
            Mode::Siege => Mode::TD,
        }
    }

    pub fn mode_discount(self, o: Self) -> f64 {
        let a = self.min(o);
        let b = self.max(o);

        match (a, b) {
            // There are very few siege and Chaser maps, deeply discount their cross-type penalty
            (Mode::Siege, Mode::Siege) => 1.0,
            (Mode::Siege, _) => 0.1,
            (_, Mode::Siege) => 0.1,

            (Mode::Chaser, Mode::Chaser) => 1.0,
            (Mode::Chaser, _) => 0.1,
            (_, Mode::Chaser) => 0.1,

            (Mode::TD, Mode::DM) => 0.6,
            (Mode::TD, Mode::BR) => 0.5,
            (Mode::TD, Mode::Captain) => 0.5,

            (Mode::DM, Mode::BR) => 0.9,
            (Mode::DM, Mode::Captain) => 0.8,

            (Mode::BR, Mode::Captain) => 0.7,

            _ => 1.0,
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Unknown variant name {0}")]
pub struct UnknownMode(String);

impl TryInto<Mode> for &str {
    type Error = UnknownMode;

    fn try_into(self) -> Result<Mode, Self::Error> {
        let lc = &*self.to_lowercase();

        match lc {
            "td" => Ok(Mode::TD),
            "dm" => Ok(Mode::DM),
            "chaser" => Ok(Mode::Chaser),
            "br" => Ok(Mode::BR),
            "captain" => Ok(Mode::Captain),
            "siege" => Ok(Mode::Siege),
            _ => Err(UnknownMode(self.to_string())),
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Mode::TD => "TD",
            Mode::DM => "DM",
            Mode::Chaser => "Chaser",
            Mode::BR => "BR",
            Mode::Captain => "Captain",
            Mode::Siege => "Siege",
        };

        self.console_color().maybe_color().paint(name).fmt(f)
    }
}

#[derive(Debug)]
pub struct MapGroup {
    pub gid: u16,
    pub basename: String,
    pub variants: Vec<Rc<Map>>,
}

impl PartialEq for MapGroup {
    fn eq(&self, other: &Self) -> bool {
        self.gid == other.gid
    }
}

impl Eq for MapGroup {}

pub struct Map {
    pub id: u16,
    group: RefCell<Option<Rc<MapGroup>>>,
    pub nickname: String,
    pub mode: Mode,
    pub players: u16,
    pub is_gag: bool,
    pub disabled: bool,
}

impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Map {}

impl std::fmt::Debug for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Map{{id: {}, group: {}, nickname: {}, mode: {}, players: {}, is_gag: {}, disabled: {} }}", 
        self.id, self.group().gid, self.nickname, self.mode, self.players, self.is_gag, self.disabled,
        )
    }
}

impl Map {
    pub fn group(&self) -> Rc<MapGroup> {
        let g = self.group.borrow();
        let g = g.as_ref();
        g.unwrap().clone()
    }

    pub fn map_info(&self) -> String {
        format!("{} {} ({})", self.nickname, self.mode, self.players)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("{1}: {0}")]
pub struct GroupError(JsonValue, String);

impl GroupError {
    fn new(j: &JsonValue, err: &str) -> Self {
        GroupError(j.clone(), err.to_string())
    }
}

#[derive(thiserror::Error, Debug)]
#[error("(gid {0}) {2}: {1}")]
pub struct MapError(u16, JsonValue, String);

impl MapError {
    fn new(gid: u16, j: &JsonValue, err: &str) -> Self {
        MapError(gid, j.clone(), err.to_string())
    }
}

pub type RcGroup = Rc<MapGroup>;
pub type Groups = HashMap<u16, RcGroup>;
pub type RcMap = Rc<Map>;
pub type Maps = HashMap<u16, RcMap>;

pub fn load_map_data() -> Result<(Groups, Maps), Box<dyn Error>> {
    let raw_json = fs::read_to_string("all_maps.json")?;
    let json = json::parse(&raw_json)?;

    let mut groups: HashMap<u16, Rc<MapGroup>> = HashMap::new();
    let mut maps: HashMap<u16, Rc<Map>> = HashMap::new();

    assert!(json.is_array(), "map file must be a list");

    for g in json.members() {
        let basename = &g["name"];
        let gid = &g["gid"];
        let variants = &g["variants"];

        let gid = gid
            .as_u16()
            .ok_or_else(|| GroupError(gid.clone(), "gid not a u16".to_string()))?;
        let basename = basename
            .as_str()
            .filter(|b| !b.is_empty())
            .ok_or_else(|| GroupError::new(basename, "basename not a string not a u16"))?
            .to_string();

        assert!(
            variants.is_array(),
            "group {} needs a list of variants",
            gid
        );

        let mut group = MapGroup {
            basename: basename.clone(),
            gid,
            variants: Vec::new(),
        };

        for v in variants.members() {
            let id = &v["id"];
            let players = &v["players"];
            let mode = &v["mode"];
            let is_gag = &v["gag"];
            let nickname = &v["nickname"];
            let disabled = &v["disabled"];

            let id = id
                .as_u16()
                .ok_or_else(|| MapError::new(gid, id, "map id must be a u16"))?;
            let players = players
                .as_u16()
                .ok_or_else(|| MapError::new(gid, players, "players id must be a u16"))?;
            let mode_i = mode
                .as_str()
                .ok_or_else(|| MapError::new(gid, mode, "map mode must be a string"))?;
            let mode: Mode = mode_i
                .try_into()
                .map_err(|_| MapError::new(gid, mode, "unknown map mode"))?;
            let is_gag = if is_gag.is_null() {
                false
            } else {
                is_gag
                    .as_bool()
                    .ok_or_else(|| MapError::new(gid, is_gag, "gag must be absent or a boolean"))?
            };

            let nickname = if nickname.is_null() {
                basename.to_string()
            } else {
                nickname
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        MapError::new(gid, nickname, "nickname must be absent or a string")
                    })?
                    .to_string()
            };

            let disabled = if disabled.is_null() {
                false
            } else {
                disabled.as_bool().ok_or_else(|| {
                    MapError::new(gid, disabled, "disabled must be absent or a boolean")
                })?
            };

            let map = Rc::new(Map {
                id,
                group: RefCell::new(None),
                players,
                mode,
                nickname,
                is_gag,
                disabled,
            });

            group.variants.push(map.clone());
            let existing = maps.insert(id, map);
            if existing.is_some() {
                Err(MapError::new(gid, &((id as i32).into()), "Dulicate map id"))?;
            }
        }

        let group = Rc::new(group);
        let existing = groups.insert(gid, group.clone());
        if existing.is_some() {
            Err(GroupError::new(
                &((gid as i32).into()),
                "Dulicate group gid",
            ))?;
        }

        for v in &group.variants {
            let mut g_mut = v.group.borrow_mut();
            *g_mut = Some(group.clone());
        }
    }

    Ok((groups, maps))
}
