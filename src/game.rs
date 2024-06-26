use serde_json::Result;
use std::{collections::HashMap, fs};

use serde::Deserialize;

pub fn load_route_from_path(path: &str) -> Result<Route> {
    let file_data = fs::read_to_string(path).expect("Unable to read file");
    let json: serde_json::Value =
        serde_json::from_str(&file_data).expect("JSON was not well formatted");

    let locations: Vec<Location> = serde_json::from_value(json["locations"].clone())?;
    let location_map: HashMap<_, _> = locations.iter().map(|l| (l.id, l)).collect();

    let route_ids: Vec<usize> = serde_json::from_value(json["route"].clone())?;

    let route: Vec<_> = route_ids
        .into_iter()
        .map(|id| (*(location_map.get(&id).unwrap())).clone())
        .collect();

    Ok(Route { route })
}

#[derive(Deserialize, Debug, Clone)]
pub struct Location {
    pub id: usize,
    pub title: String,
    pub clue: String,
    pub answer: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Route {
    pub route: Vec<Location>,
}
