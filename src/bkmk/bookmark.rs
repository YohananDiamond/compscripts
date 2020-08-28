use std::cmp::Ordering;
use core::data::{Searchable, Id};
use serde::{Deserialize, Serialize};
use select::document::Document;
use select::node::Data;
use select::predicate::Name;
use curl::easy::Easy;

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

pub fn url_get_title(url: &str) -> Result<String, String> {
    let mut vec = Vec::new();

    let mut easy = Easy::new();

    if let Err(e) = easy.url(url) {
        return Err(format!("{}", e));
    }

    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            vec.extend_from_slice(data);
            Ok(data.len())
        }).unwrap(); // TODO: switch for an "expect"

        transfer.perform().unwrap(); // TODO: should I really unwrap this?
    }

    let code = easy.response_code().unwrap();
    match code {
        300..=399 => return Err(format!("got redirection code {}", code)),
        400..=499 => return Err(format!("got client error code {}", code)),
        500..=599 => return Err(format!("got server error code {}", code)),
        _ => (),
    }

    let document = match Document::from_read(String::from_utf8_lossy(&vec).as_bytes()) {
        Ok(doc) => doc,
        Err(err) => return Err(format!("IO Error: {}", err)),
    };

    let titles: Vec<_> = document.find(Name("title")).collect();

    if titles.len() == 0 {
        Err(String::from("Couldn't find any <title> tags in page"))
    } else {
        // get the first title tag, ignore the rest
        let children = titles[0]
            .children()
            .filter(|x| match x.data() {
                Data::Text(_) => true,
                _ => false,
            })
            .collect::<Vec<_>>();

        if children.len() == 0 {
            Err(String::from("Empty <title> tag found"))
        } else {
            // there's no unwrap here, so I guess I need to use this syntax.
            Ok(String::from(children[0].as_text().unwrap()))
        }
    }
}
