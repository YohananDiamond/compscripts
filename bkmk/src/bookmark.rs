use curl::easy::Easy;
use select::document::Document;
use select::predicate::Name;
use serde::{Deserialize, Serialize};

use std::cmp::Ordering;
use std::fmt::Display;

use utils::data::{Id, Searchable};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Bookmark {
    pub id: u32,
    pub archived: bool,
    pub name: String,
    pub url: String,
    pub tags: Vec<String>,
}

impl Ord for Bookmark {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Bookmark {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Searchable for Bookmark {
    fn ref_id(&self) -> Option<Id> {
        Some(self.id)
    }
}

pub fn url_get_title(url: &str) -> Result<String, Box<dyn Display + 'static>> {
    let mut vec = Vec::new();

    let mut easy = Easy::new();

    easy.url(url)
        .map_err(|why| Box::new(format!("Curl error: {}", why)) as _)?;

    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                vec.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();

        transfer
            .perform()
            .map_err(|_| Box::new("Failed to download/write to buffer") as _)?;
    }

    let code = easy.response_code().unwrap();
    match code {
        300..=399 => return Err(Box::new(format!("got redirection code {}", code))), // TODO: parse redirection codes
        400..=499 => return Err(Box::new(format!("got client error code {}", code))),
        500..=599 => return Err(Box::new(format!("got server error code {}", code))),
        _ => (),
    }

    let document = Document::from_read(String::from_utf8_lossy(&vec).as_bytes())
        .map_err(|why| Box::new(format!("Failed to parse webpage: {}", why)) as _)?;

    if let Some(title_tag) = document.find(Name("title")).nth(0) {
        // get the first text element of the title tag (can there even be more than that?), ignore the rest
        if let Some(title) = title_tag
            .children()
            .filter_map(|node| node.as_text())
            .next()
        {
            Ok(title.to_string())
        } else {
            Err(Box::new("Empty <title> tag"))
        }
    } else {
        Err(Box::new("Couldn't find any <title> tags in page"))
    }
}
