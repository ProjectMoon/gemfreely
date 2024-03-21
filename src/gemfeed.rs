use chrono::{DateTime, NaiveDateTime, Timelike, Utc};
use once_cell::sync::OnceCell;
use regex::Regex;
use std::borrow::Cow;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::slice::IterMut;

use anyhow::{anyhow, Error, Result};
use atom_syndication::{Entry as AtomEntry, Feed as AtomFeed};
use germ::ast::{Ast as GemtextAst, Node as GemtextNode};
use germ::convert::{self as germ_convert, Target};
use germ::request::{request as gemini_request, Response as GeminiResponse};
use url::Url;

fn parse_gemfeed_gemtext(base_url: &Url, gemfeed: &GemtextAst) -> Vec<GemfeedEntry> {
    gemfeed
        .inner()
        .into_iter()
        .filter_map(|node| GemfeedEntry::from_ast(base_url, node))
        .collect()
}

fn parse_gemfeed_atom(feed: &str) -> Result<Vec<GemfeedEntry>> {
    let feed = feed.parse::<AtomFeed>()?;

    let entries = feed
        .entries()
        .into_iter()
        .filter_map(|entry| GemfeedEntry::from_atom(entry))
        .collect::<Vec<_>>();

    Ok(entries)
}

enum GemfeedType {
    Gemtext,
    Atom,
    Unknown,
}

impl GemfeedType {
    const ATOM_MIME_TYPES: &'static [&'static str] = &["text/xml", "application/atom+xml"];
}

impl From<Cow<'_, str>> for GemfeedType {
    // See https://github.com/gemrest/germ/issues/2. Will be converted
    // to use germ Meta struct after this is fixed.
    fn from(mime: Cow<'_, str>) -> Self {
        let is_atom = Self::ATOM_MIME_TYPES
            .into_iter()
            .any(|atom_mime| mime.contains(atom_mime));

        if is_atom {
            GemfeedType::Atom
        } else if mime.contains("text/gemini") {
            GemfeedType::Gemtext
        } else {
            GemfeedType::Unknown
        }
    }
}

#[derive(Debug)]
pub struct Gemfeed {
    url: Url,
    entries: Vec<GemfeedEntry>,
}

#[allow(dead_code)]
impl Gemfeed {
    pub fn new(url: &Url, entries: Vec<GemfeedEntry>) -> Gemfeed {
        Gemfeed {
            url: url.clone(),
            entries,
        }
    }

    pub fn load(url: &Url) -> Result<Gemfeed> {
        let resp = gemini_request(url)?;
        match GemfeedType::from(resp.meta()) {
            GemfeedType::Gemtext => Self::load_from_gemtext(url, resp),
            GemfeedType::Atom => Self::load_from_atom(url, resp),
            _ => Err(anyhow!(
                "Unrecognized Gemfeed mime type [meta={}]",
                resp.meta()
            )),
        }
    }

    fn load_from_atom(url: &Url, resp: GeminiResponse) -> Result<Gemfeed> {
        if let Some(content) = resp.content() {
            let entries = parse_gemfeed_atom(content)?;
            Ok(Self::new(url, entries))
        } else {
            Err(anyhow!("Not a valid Atom Gemfeed"))
        }
    }

    fn load_from_gemtext(url: &Url, resp: GeminiResponse) -> Result<Gemfeed> {
        let maybe_feed = resp
            .content()
            .to_owned()
            .map(|text| GemtextAst::from_value(&text));

        // TODO should be some actual validation of the feed here.
        if let Some(ref feed) = maybe_feed {
            let entries = parse_gemfeed_gemtext(url, feed);
            Ok(Self::new(url, entries))
        } else {
            Err(anyhow!("Not a valid Gemtextg Gemfeed"))
        }
    }

    pub fn slugs(&self) -> Vec<String> {
        self.entries()
            .map(|entry| entry.slug().to_owned())
            .collect()
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn entries(&self) -> impl Iterator<Item = &GemfeedEntry> {
        self.entries.iter()
    }

    pub fn entries_mut(&mut self) -> IterMut<GemfeedEntry> {
        self.entries.iter_mut()
    }

    pub fn find_entry_by_slug<S: AsRef<str>>(&self, slug: S) -> Option<&GemfeedEntry> {
        let slug = slug.as_ref();
        self.entries().find(|entry| entry.slug() == slug)
    }

    pub fn find_mut_entry_by_slug<S: AsRef<str>>(&mut self, slug: S) -> Option<&mut GemfeedEntry> {
        let slug = slug.as_ref();
        self.entries_mut().find(|entry| entry.slug() == slug)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct GemfeedEntry {
    title: String,
    slug: String,
    published: Option<DateTime<Utc>>,

    /// Full URL of the gemlog post.
    url: Url,

    /// Must be loaded by calling the body() method.
    body: OnceCell<String>,
}

#[allow(dead_code)]
impl GemfeedEntry {
    pub fn from_ast(base_url: &Url, node: &GemtextNode) -> Option<GemfeedEntry> {
        let link = GemfeedLink::try_from(node).ok()?;
        // Gemfeeds have only the date--lock to 12pm UTC as a guess.
        let publish_date = link
            .published
            .map(|date| NaiveDateTime::parse_from_str(&date, "%Y-%m-%d"))?
            .ok()?
            .with_hour(12)?
            .and_utc();

        Some(GemfeedEntry {
            title: link.title,
            url: base_url.join(&link.path).ok()?,
            slug: link.slug,
            published: Some(publish_date),
            body: OnceCell::new(),
        })
    }

    pub fn from_atom(entry: &AtomEntry) -> Option<GemfeedEntry> {
        let link = GemfeedLink::try_from(entry).ok()?;
        let publish_date = link
            .published
            .map(|date| DateTime::parse_from_rfc3339(&date))?
            .ok()?
            .to_utc();

        Some(GemfeedEntry {
            title: link.title,
            url: Url::parse(&link.path).ok()?,
            slug: link.slug,
            published: Some(publish_date),
            body: OnceCell::new(),
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn published(&self) -> Option<&DateTime<Utc>> {
        self.published.as_ref()
    }

    /// Full URL of the gemlog post.
    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn body(&self) -> Result<&String, Error> {
        self.body.get_or_try_init(|| {
            let resp = gemini_request(&self.url)?;
            Ok(resp.content().to_owned().unwrap_or_default())
        })
    }

    pub fn body_mut(&mut self) -> Result<&mut String, Error> {
        // Forces init and also returns the error if init failed.
        if let Err(error) = self.body() {
            return Err(error);
        }

        // Which means that this Should Be Safe™
        Ok(self
            .body
            .get_mut()
            .expect("Body not initialized when it should be"))
    }

    /// The gemtext body of the gemlog post, represented as a
    /// germ::Ast. The body is loaded lazily when this method is first
    /// called.
    pub fn body_as_ast(&self) -> Result<GemtextAst, Error> {
        self.body().map(|text| GemtextAst::from_value(&text))
    }

    pub fn body_as_markdown(&self) -> Result<String, Error> {
        self.body_as_ast()
            .map(|body| germ_convert::from_ast(&body, &Target::Markdown))
    }
}

struct GemfeedLink {
    path: String,
    title: String,
    slug: String,
    published: Option<String>,
}

impl TryFrom<&GemtextNode> for GemfeedLink {
    type Error = anyhow::Error;
    fn try_from(node: &GemtextNode) -> StdResult<Self, Self::Error> {
        let entry: Option<GemfeedLink> = if let GemtextNode::Link {
            text: Some(title),
            to: path,
        } = node.to_owned()
        {
            let re = Regex::new(r#"(\d\d\d\d-\d\d-\d\d)"#).unwrap();
            let path_buf = PathBuf::from(&path);

            let published: Option<String> = re
                .captures_at(&title, 0)
                .map(|caps| caps.get(0))
                .and_then(|date| date.map(|published| published.as_str().to_owned()));

            let stem = match published {
                Some(_) => path_buf.file_stem(),
                _ => None,
            };

            let maybe_slug = stem.map(|s| s.to_string_lossy());
            maybe_slug.map(|slug| GemfeedLink {
                title,
                path,
                published,
                slug: slug.to_string(),
            })
        } else {
            None
        };

        entry.ok_or(anyhow!("Not a Gemfeed link"))
    }
}

impl TryFrom<&AtomEntry> for GemfeedLink {
    type Error = anyhow::Error;

    fn try_from(entry: &AtomEntry) -> StdResult<Self, Self::Error> {
        let link = entry
            .links()
            .iter()
            .find(|link| link.rel == "alternate")
            .map(|link| link.href.clone())
            .ok_or(anyhow!("No post link present"))?;

        let link_url = Url::parse(&link)?;
        let post_filename = link_url
            .path_segments()
            .and_then(|segments| segments.last())
            .map(|filename| PathBuf::from(filename));

        let maybe_slug = match post_filename {
            Some(ref pathbuf) => pathbuf
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string()),
            _ => None,
        };

        let title = entry.title().to_string();
        let published = entry.published();

        if let Some(slug) = maybe_slug {
            Ok(GemfeedLink {
                path: link.clone(),
                published: published.map(|date| date.to_string()),
                title,
                slug,
            })
        } else {
            Err(anyhow!("Slug could not be calculated: [url={}]", link_url))
        }
    }
}
