use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    rc::Rc,
};

use chrono::Utc;
use regex::Regex;

use crate::map_data::{Map, Maps};

pub fn append_log(map: &Map) -> Result<(), Box<dyn Error>> {
    let mut option = OpenOptions::new();
    option.read(true);
    option.append(true);
    option.create(true);

    let mut f = option.open("play_log.txt")?;

    let pos = f.seek(SeekFrom::End(0))?;

    let mut last = String::new();
    if pos > 0 {
        f.seek(SeekFrom::End(-1))?;
        f.read_to_string(&mut last)?;

        f.seek(SeekFrom::End(0))?;

        if last != "\n" {
            f.write_all("\n".as_bytes())?;
        }
    }

    let now = Utc::now();
    let now = now.format("%Y-%m-%d %H:%M Z").to_string();

    f.write_fmt(format_args!(
        "#{} ({}) {} {}\n",
        map.id, now, map.nickname, map.mode
    ))?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
#[error("Error Parsing the log at line {0}, {1}: '{2}'")]
pub struct LogError(i32, String, String);

impl LogError {
    fn new<E, V>(line_num: i32, err: E, val: V) -> Self
    where
        E: ToString,
        V: ToString,
    {
        LogError(line_num, err.to_string(), val.to_string())
    }
}

pub fn load_log(maps: &Maps) -> Result<Vec<Rc<Map>>, Box<dyn Error>> {
    let mut option = OpenOptions::new();
    option.read(true);
    option.append(true);
    option.create(true);

    let f = option.open("play_log.txt")?;
    let reader = BufReader::new(f);

    let mut records = Vec::new();

    for (line, line_num) in reader.lines().zip(1..) {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue; // ignore empty lines
        }

        let re = Regex::new("\\d{1,3}")?;
        let ma = re.find(line);
        if ma.is_none() {
            return Err(Box::new(LogError::new(
                line_num,
                "Could not find map id",
                line,
            )));
        }

        let ma = ma.unwrap().as_str().to_string();
        let id = ma
            .parse::<u16>()
            .map_err(|_| LogError::new(line_num, "Could not parse map id", line))?;

        let map = maps.get(&id);
        if map.is_none() {
            return Err(Box::new(LogError::new(
                line_num,
                "Could not find map with id",
                id.to_string(),
            )));
        }

        records.push(map.unwrap().clone());
    }

    Ok(records)
}
