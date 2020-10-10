use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
};

use crate::network::{TopicManifest, TopicManifests};
use crate::parser::list_installed;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, to_string};

const SOURCE_HEADER: &[u8] = b"# Generated by AOSC Topic Manager. DO NOT EDIT THIS FILE!\n";
const SOURCE_PATH: &str = "/etc/apt/sources.list.d/atm.list";
const STATE_PATH: &str = "/var/lib/atm/state";
const STATE_DIR: &str = "/var/lib/atm/";
const DPKG_STATE: &str = "/var/lib/dpkg/state";
const MIRROR_URL: &str = "https://repo.aosc.io/debs";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PreviousTopic {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub date: i64,
    pub packages: Vec<String>,
}

type PreviousTopics = Vec<PreviousTopic>;

pub fn get_closed_topic(current: &TopicManifests) -> Result<PreviousTopics> {
    let mut current_topics: HashSet<String> = HashSet::new();
    let mut closed_topics = Vec::new();
    for topic in current {
        current_topics.insert(topic.name.clone());
    }
    for topic in get_previous_topics()? {
        if !current_topics.contains(&topic.name) {
            closed_topics.push(topic);
        }
    }

    Ok(closed_topics)
}

/// Returns the packages need to be reinstalled
pub fn close_topics(topics: &TopicManifests) -> Result<Vec<String>> {
    let state_file = fs::read(DPKG_STATE)?;
    let installed = list_installed(&state_file)?;
    let mut remove = Vec::new();

    for topic in topics {
        for package in topic.packages.iter() {
            if installed.contains(package) {
                remove.push(format!("{}/stable", package));
            }
        }
    }

    Ok(remove)
}

fn get_previous_topics() -> Result<PreviousTopics> {
    let f = std::fs::read(STATE_PATH)?;

    Ok(from_slice(&f)?)
}

pub fn get_display_listing(current: TopicManifests) -> TopicManifests {
    let prev = get_previous_topics().unwrap_or(vec![]);
    let mut lookup: HashMap<String, TopicManifest> = HashMap::new();
    let current_len = current.len();

    for topic in current.into_iter() {
        lookup.insert(topic.name.clone(), topic);
    }

    let mut concatenated = Vec::new();
    concatenated.reserve(prev.len() + current_len);
    for topic in prev {
        if let Some(topic) = lookup.get_mut(&topic.name) {
            topic.enabled = true;
            continue;
        }
        concatenated.push(TopicManifest {
            enabled: false,
            closed: true,
            name: topic.name.clone(),
            description: topic.description.clone(),
            date: topic.date,
            arch: HashSet::new(),
            packages: topic.packages.clone(),
        });
    }
    // consume the lookup table and append all the elements to the concatenated list
    for topic in lookup.into_iter() {
        concatenated.push(topic.1);
    }

    concatenated
}

fn save_as_previous_topics(current: &[&TopicManifest]) -> Result<String> {
    let mut previous_topics = Vec::new();
    for topic in current {
        if !topic.enabled {
            continue;
        }
        previous_topics.push(PreviousTopic {
            name: topic.name.clone(),
            description: topic.description.clone(),
            date: topic.date,
            packages: topic.packages.clone(),
        });
    }

    Ok(to_string(&previous_topics)?)
}

fn make_topic_list(topics: &[&TopicManifest]) -> String {
    let mut output = String::new();
    output.reserve(1024);

    for topic in topics {
        output.push_str(&format!(
            "# Topic `{}`\ndeb {} {} main\n",
            topic.name, MIRROR_URL, topic.name
        ));
    }

    output
}

pub fn write_source_list(topics: &[&TopicManifest]) -> Result<()> {
    let mut f = std::fs::File::create(SOURCE_PATH)?;
    f.write(SOURCE_HEADER)?;
    f.write(make_topic_list(topics).as_bytes())?;

    std::fs::create_dir_all(STATE_DIR)?;
    let mut f = std::fs::File::create(STATE_PATH)?;
    f.write(save_as_previous_topics(topics)?.as_bytes())?;

    Ok(())
}
