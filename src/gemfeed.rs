use chrono::{DateTime, NaiveDate, Utc};
use once_cell::sync::{Lazy, OnceCell};
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

use crate::Cli;

static GEMFEED_POST_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| Regex::new(r#"(\d\d\d\d-\d\d-\d\d)"#).unwrap());

fn is_gemfeed_post_link(node: &GemtextNode) -> bool {
    if let GemtextNode::Link {
        text: Some(title), ..
    } = node
    {
        GEMFEED_POST_REGEX.is_match_at(title, 0)
    } else {
        false
    }
}

fn parse_gemfeed(base_url: &Url, gemfeed: &GemtextAst) -> Result<Vec<GemfeedEntry>> {
    gemfeed
        .inner()
        .into_iter()
        .filter(|node| is_gemfeed_post_link(node))
        .map(|node| GemfeedEntry::from_ast(base_url, node))
        .collect()
}

fn parse_atom(
    feed: &AtomFeed,
    settings: &GemfeedParserSettings,
) -> Result<Vec<GemfeedEntry>> {
    feed.entries()
        .into_iter()
        .map(|entry| GemfeedEntry::from_atom(entry, &settings.atom_date_format))
        .collect()
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

#[allow(dead_code)]
#[derive(Debug)]
pub struct Gemfeed {
    url: Url,
    title: String,
    entries: Vec<GemfeedEntry>,
}

/// Settings for controlling how the Gemfeed is parsed.
pub struct GemfeedParserSettings<'a> {
    atom_date_format: &'a str,
}

impl GemfeedParserSettings<'_> {
    const DEFAULT_DATE_FORMAT: &'static str = "%Y-%m-%d %H:%M:%S %:z";
}

impl<'a> From<&'a Cli> for GemfeedParserSettings<'a> {
    fn from(cli: &'a Cli) -> Self {
        cli.date_format
            .as_deref()
            .map(|date_fmt| GemfeedParserSettings {
                atom_date_format: date_fmt,
            })
            .unwrap_or(Self::default())
    }
}

impl Default for GemfeedParserSettings<'_> {
    fn default() -> Self {
        GemfeedParserSettings {
            atom_date_format: Self::DEFAULT_DATE_FORMAT,
        }
    }
}

#[allow(dead_code)]
impl Gemfeed {
    pub fn new(url: &Url, title: &str, entries: Vec<GemfeedEntry>) -> Gemfeed {
        Gemfeed {
            url: url.clone(),
            title: title.to_owned(),
            entries,
        }
    }

    pub fn load(url: &Url) -> Result<Gemfeed> {
        Self::load_with_settings(url, &GemfeedParserSettings::default())
    }

    pub fn load_with_settings(url: &Url, settings: &GemfeedParserSettings) -> Result<Gemfeed> {
        let resp = gemini_request(url)?;
        match GemfeedType::from(resp.meta()) {
            GemfeedType::Gemtext => Self::load_from_gemtext(url, resp),
            GemfeedType::Atom => Self::load_from_atom(url, resp, &settings),
            _ => Err(anyhow!(
                "Unrecognized Gemfeed mime type [meta={}]",
                resp.meta()
            )),
        }
    }

    fn load_from_atom(
        url: &Url,
        resp: GeminiResponse,
        settings: &GemfeedParserSettings,
    ) -> Result<Gemfeed> {
        if let Some(content) = resp.content() {
            let feed = content.parse::<AtomFeed>()?;
            let entries = parse_atom(&feed, settings)?;
            let title = feed.title();
            Ok(Self::new(url, title, entries))
        } else {
            Err(anyhow!("Not a valid Atom Gemfeed"))
        }
    }

    fn load_from_gemtext(url: &Url, resp: GeminiResponse) -> Result<Gemfeed> {
        let maybe_feed = resp
            .content()
            .to_owned()
            .map(|text| GemtextAst::from_value(&text));

        if let Some(ref feed) = maybe_feed {
            Self::load_from_ast(url, feed)
        } else {
            Err(anyhow!("Not a valid Gemfeed - could not parse gemtext"))
        }
    }

    fn load_from_ast(url: &Url, feed: &GemtextAst) -> Result<Gemfeed> {
        let feed_title = feed.inner().iter().find_map(|node| match node {
            GemtextNode::Heading { level, text } if *level == (1 as usize) => Some(text),
            _ => None,
        });

        if let Some(title) = feed_title {
            let entries = parse_gemfeed(url, feed)?;
            Ok(Self::new(url, title, entries))
        } else {
            Err(anyhow!("Not a valid Gemfeed: missing title"))
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
    pub fn from_ast(base_url: &Url, node: &GemtextNode) -> Result<GemfeedEntry> {
        let link = GemfeedLink::try_from(node)?;
        // Gemfeeds have only the date--according to spec, it should be 12pm UTC.
        println!("{:?}", link.published);
        let publish_date = link
            .published
            .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d"))
            .ok_or(anyhow!("No publish date found"))??
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();

        Ok(GemfeedEntry {
            title: link.title,
            url: base_url.join(&link.path)?,
            slug: link.slug,
            published: Some(publish_date),
            body: OnceCell::new(),
        })
    }

    pub fn from_atom(entry: &AtomEntry, date_format: &str) -> Result<GemfeedEntry> {
        let link = GemfeedLink::try_from(entry)?;

        let publish_date = link
            .published
            .ok_or(anyhow!("No publish date found"))
            .map(|date| DateTime::parse_from_str(&date, date_format))??
            .to_utc();

        Ok(GemfeedEntry {
            title: link.title,
            url: Url::parse(&link.path)?,
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

            // Strip the date from the title, if possible.
            let title = published
                .as_ref()
                .and_then(|date| title.strip_prefix(&*date))
                .map(|text| text.trim())
                .unwrap_or(&title);

            let maybe_slug = stem.map(|s| s.to_string_lossy());
            maybe_slug.map(|slug| GemfeedLink {
                title: title.to_string(),
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

#[cfg(test)]
mod gemfeed_tests {
    use super::*;

    #[test]
    fn parse_gemfeed_invalid_if_no_title() -> Result<()> {
        let gemfeed: String = r#"
        This is a gemfeed without a title.
        => atom.xml Atom Feed

        ## Posts

        => post2.gmi 2023-03-05 Post 2
        => post1.gmi 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let base_url = Url::parse("gemini://example.com/posts")?;
        let ast = GemtextAst::from_string(gemfeed);
        let result = Gemfeed::load_from_ast(&base_url, &ast);
        assert!(matches!(result, Err(_)));
        Ok(())
    }

    #[test]
    fn parse_gemfeed_ignores_non_post_links() -> Result<()> {
        let gemfeed: String = r#"
        # My Gemfeed

        This is a gemfeed.
        => atom.xml Atom Feed

        ## Posts

        => post2.gmi 2023-03-05 Post 2
        => post1.gmi 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let base_url = Url::parse("gemini://example.com/posts")?;
        let ast = GemtextAst::from_string(gemfeed);
        let results = parse_gemfeed(&base_url, &ast)?;
        assert_eq!(results.len(), 2);
        Ok(())
    }

    #[test]
    fn parse_gemfeed_ignores_non_links() -> Result<()> {
        let gemfeed: String = r#"
        # My Gemfeed

        This is a gemfeed.

        ## Posts

        => post2.gmi 2023-03-05 Post 2
        => post1.gmi 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let base_url = Url::parse("gemini://example.com/posts")?;
        let ast = GemtextAst::from_string(gemfeed);
        let results = parse_gemfeed(&base_url, &ast)?;
        assert_eq!(results.len(), 2);
        Ok(())
    }

    #[test]
    fn convert_gemfeed_links_success() -> Result<()> {
        let gemfeed_links: String = r#"
        => post2.gmi 2023-03-05 Post 2
        => post1.gmi 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let ast = GemtextAst::from_string(gemfeed_links);

        let result = ast
            .inner()
            .into_iter()
            .map(|node| GemfeedLink::try_from(node))
            .flat_map(|res| res.ok())
            .collect::<Vec<_>>();

        let expected = vec![
            GemfeedLink {
                path: "post2.gmi".into(),
                slug: "post2".into(),
                title: "Post 2".into(),
                published: Some("2023-03-05".into()),
            },
            GemfeedLink {
                path: "post1.gmi".into(),
                slug: "post1".into(),
                title: "Post 1".into(),
                published: Some("2023-02-01".into()),
            },
        ];

        assert_eq!(expected, result);
        Ok(())
    }

    fn slug_test(gemtext: String, expected_slugs: Vec<String>) -> Result<()> {
        let ast = GemtextAst::from_string(gemtext);

        let result = ast
            .inner()
            .into_iter()
            .map(|node| GemfeedLink::try_from(node))
            .flat_map(|res| res.ok())
            .map(|link| link.slug)
            .collect::<Vec<_>>();

        assert_eq!(expected_slugs, result);
        Ok(())
    }

    #[test]
    fn convert_gemfeed_slug_with_slash() -> Result<()> {
        let gemfeed_links: String = r#"
        => ./post2 2023-03-05 Post 2
        => ./post1 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let expected = vec!["post2".into(), "post1".into()];
        slug_test(gemfeed_links, expected)
    }

    #[test]
    fn convert_gemfeed_slug_no_ext() -> Result<()> {
        let gemfeed_links: String = r#"
        => post2 2023-03-05 Post 2
        => post1 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let expected = vec!["post2".into(), "post1".into()];
        slug_test(gemfeed_links, expected)
    }

    #[test]
    fn convert_gemfeed_slug_no_ext_with_slash() -> Result<()> {
        let gemfeed_links: String = r#"
        => ./post2 2023-03-05 Post 2
        => ./post1 2023-02-01 Post 1
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let expected = vec!["post2".into(), "post1".into()];
        slug_test(gemfeed_links, expected)
    }
}

#[cfg(test)]
mod atom_tests {
    use atom_syndication::FixedDateTime;
    use once_cell::sync::Lazy;

    use super::*;

    const ATOM_DATE_FORMAT: &'static str = GemfeedParserSettings::DEFAULT_DATE_FORMAT;

    static ATOM_DATE: Lazy<FixedDateTime> = Lazy::new(|| {
        FixedDateTime::parse_from_str("2024-03-01 20:30:00 +01:00", ATOM_DATE_FORMAT).unwrap()
    });

    #[test]
    fn convert_atom_entry_success() {
        let entry = AtomEntry {
            title: "TestTitle".into(),
            published: Some(ATOM_DATE.to_owned()),
            links: vec![atom_syndication::Link {
                href: "gemini://example.com/posts/test.gmi".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = GemfeedLink::try_from(&entry);
        let expected = GemfeedLink {
            path: "gemini://example.com/posts/test.gmi".into(),
            published: Some("2024-03-01 20:30:00 +01:00".to_string()),
            slug: "test".into(),
            title: "TestTitle".into(),
        };

        assert_eq!(result.ok(), Some(expected));
    }

    #[test]
    fn convert_atom_entry_no_file_ext() {
        let entry = AtomEntry {
            title: "TestTitle".into(),
            published: Some(ATOM_DATE.to_owned()),
            links: vec![atom_syndication::Link {
                href: "gemini://example.com/posts/test".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = GemfeedLink::try_from(&entry);
        let expected = GemfeedLink {
            path: "gemini://example.com/posts/test".into(),
            published: Some("2024-03-01 20:30:00 +01:00".to_string()),
            slug: "test".into(),
            title: "TestTitle".into(),
        };

        assert_eq!(result.ok(), Some(expected));
    }

    #[test]
    fn convert_atom_entry_no_date() {
        let entry = AtomEntry {
            title: "TestTitle".into(),
            published: None,
            links: vec![atom_syndication::Link {
                href: "gemini://example.com/posts/test.gmi".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = GemfeedLink::try_from(&entry);
        let expected = GemfeedLink {
            path: "gemini://example.com/posts/test.gmi".into(),
            published: None,
            slug: "test".into(),
            title: "TestTitle".into(),
        };

        assert_eq!(result.ok(), Some(expected));
    }

    #[test]
    fn convert_atom_entry_no_link() {
        let entry = AtomEntry {
            title: "TestTitle".into(),
            published: None,
            links: vec![],
            ..Default::default()
        };

        let result = GemfeedLink::try_from(&entry);
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn convert_atom_entry_invalid_link() {
        let entry = AtomEntry {
            title: "TestTitle".into(),
            published: None,
            links: vec![atom_syndication::Link {
                href: "example.com/posts/test.gmi".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = GemfeedLink::try_from(&entry);
        assert!(matches!(result, Err(_)));
    }
}
