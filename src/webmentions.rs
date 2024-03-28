use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset};
use url::Url;

const WEBMENTIONS_IO_ENDPOINT: &'static str = "/api/mentions.jf2";

trait ToQueryPair<T> {
    fn to_query_pair(&self) -> T;
}

pub(crate) enum WebmentionsSince {
    SinceId(usize),
    SinceDate(DateTime<FixedOffset>),
}

impl ToQueryPair<(String, String)> for WebmentionsSince {
    fn to_query_pair(&self) -> (String, String) {
        match self {
            Self::SinceId(id) => ("since_id".to_string(), id.to_string()),
            Self::SinceDate(date) => ("since".to_string(), format!("{}", date.format("%FT%T%z"))),
        }
    }
}

pub(crate) enum WebmentionType {
    InReplyTo,
    LikeOf,
    RepostOf,
    BookmarkOf,
    MentionOf,
    Rsvp,
}

impl ToString for WebmentionType {
    fn to_string(&self) -> String {
        match self {
            Self::InReplyTo => "in-reply-to".to_string(),
            Self::LikeOf => "like-of".to_string(),
            Self::RepostOf => "repost-of".to_string(),
            Self::BookmarkOf => "bookmark-of".to_string(),
            Self::MentionOf => "mention-of".to_string(),
            Self::Rsvp => "rsvp".to_string(),
        }
    }
}

impl ToQueryPair<Vec<(String, String)>> for Vec<WebmentionType> {
    fn to_query_pair(&self) -> Vec<(String, String)> {
        self.iter()
            .map(|mention_type| ("wm-property[]".to_string(), mention_type.to_string()))
            .collect()
    }
}

impl<'a> ToQueryPair<Vec<(String, String)>> for &'a [WebmentionType] {
    fn to_query_pair(&self) -> Vec<(String, String)> {
        self.iter()
            .map(|mention_type| ("wm-property[]".to_string(), mention_type.to_string()))
            .collect()
    }
}

impl ToQueryPair<(String, String)> for WebmentionType {
    fn to_query_pair(&self) -> (String, String) {
        ("wm-property".to_string(), self.to_string())
    }
}

enum NumWebmentionTypes<'a> {
    Single(&'a WebmentionType),
    Multiple(&'a [WebmentionType]),
    Zero,
}

pub(crate) struct GetWebmentionsRequest {
    /// If specified, retrieve webmentions since an ID or date/time.
    /// If not specified, fetch all possible webmentions from the
    /// server.
    since: Option<WebmentionsSince>,

    /// If specified, fetch only these types of web mentions. An empty
    /// vec will result in no webmentions fetched. If not specified,
    /// fetch all kinds of webmentions.
    types: Option<Vec<WebmentionType>>,
}

impl GetWebmentionsRequest {
    fn types(&self) -> Option<NumWebmentionTypes> {
        self.types.as_ref().map(|types| {
            if types.len() > 1 {
                NumWebmentionTypes::Multiple(types.as_slice())
            } else if types.len() == 1 {
                NumWebmentionTypes::Single(types.first().unwrap())
            } else {
                NumWebmentionTypes::Zero
            }
        })
    }
}

fn create_querystring(req: &GetWebmentionsRequest) -> Result<String> {
    let mut query_pairs: Vec<String> = vec![];
    if let Some((key, value)) = req.since.as_ref().map(|s| s.to_query_pair()) {
        query_pairs.push(format!("{}={}", &key, &value));
    }

    if let Some(num_types) = req.types() {
        let pairs = match num_types {
            NumWebmentionTypes::Multiple(types) => types.to_query_pair(),
            NumWebmentionTypes::Single(wm_type) => vec![wm_type.to_query_pair()],
            _ => {
                return Err(anyhow!(
                    "Webmention types filter specified, but no types given"
                ))
            }
        };

        for (key, value) in pairs {
            query_pairs.push(format!("{}={}", &key, &value));
        }
    }

    if query_pairs.len() > 1 {
        Ok(query_pairs.join("&"))
    } else if query_pairs.len() == 1 {
        Ok(query_pairs.swap_remove(0))
    } else {
        Ok("".to_string())
    }
}

fn create_request_url(base_url: &Url, req: &GetWebmentionsRequest) -> Result<Url> {
    let mut url = base_url.join(WEBMENTIONS_IO_ENDPOINT)?;
    let mut querystring = create_querystring(req)?;
    url.set_query(Some(&querystring));

    Ok(url)
}

pub(crate) struct WebmentionIoClient {
    url: Url,
    domain: String,
}

impl WebmentionIoClient {
    pub async fn get_mentions(params: GetWebmentionsRequest) {
        //
    }
}

#[cfg(test)]
mod create_querystring_tests {
    use super::*;

    #[test]
    fn create_querystring_with_since_date() -> Result<()> {
        let date = DateTime::parse_from_str("2022-03-08T13:05:27-0100", "%FT%T%z")?;

        let req = GetWebmentionsRequest {
            since: Some(WebmentionsSince::SinceDate(date)),
            types: None,
        };

        let expected = "since=2022-03-08T13:05:27-0100";
        let querystring = create_querystring(&req)?;
        assert_eq!(querystring.as_str(), expected);

        Ok(())
    }

    #[test]
    fn create_querystring_with_since_id() -> Result<()> {
        let req = GetWebmentionsRequest {
            since: Some(WebmentionsSince::SinceId(12345)),
            types: None,
        };

        let expected = "since_id=12345";
        let querystring = create_querystring(&req)?;
        assert_eq!(querystring, expected);

        Ok(())
    }

    #[test]
    fn create_querystring_with_one_type() -> Result<()> {
        let req = GetWebmentionsRequest {
            since: None,
            types: Some(vec![WebmentionType::InReplyTo]),
        };

        let expected = "wm-property=in-reply-to";
        let querystring = create_querystring(&req)?;
        assert_eq!(querystring, expected);

        Ok(())
    }

    #[test]
    fn create_querystring_with_since_and_one_type() -> Result<()> {
        let req = GetWebmentionsRequest {
            since: Some(WebmentionsSince::SinceId(12345)),
            types: Some(vec![WebmentionType::InReplyTo]),
        };

        let expected = "since_id=12345&wm-property=in-reply-to";
        let querystring = create_querystring(&req)?;
        assert_eq!(querystring, expected);

        Ok(())
    }

    #[test]
    fn create_querystring_with_mutiple_types() -> Result<()> {
        let req = GetWebmentionsRequest {
            since: None,
            types: Some(vec![WebmentionType::InReplyTo, WebmentionType::BookmarkOf]),
        };

        let expected = "wm-property[]=in-reply-to&wm-property[]=bookmark-of";
        let querystring = create_querystring(&req)?;
        assert_eq!(querystring, expected);

        Ok(())
    }

    #[test]
    fn create_querystring_with_since_and_mutiple_types() -> Result<()> {
        let req = GetWebmentionsRequest {
            since: Some(WebmentionsSince::SinceId(12345)),
            types: Some(vec![WebmentionType::InReplyTo, WebmentionType::BookmarkOf]),
        };

        let expected = "since_id=12345&wm-property[]=in-reply-to&wm-property[]=bookmark-of";
        let querystring = create_querystring(&req)?;
        assert_eq!(querystring, expected);

        Ok(())
    }

    #[test]
    fn create_querystring_with_no_types_in_filter() -> Result<()> {
        let req = GetWebmentionsRequest {
            since: None,
            types: Some(vec![]),
        };

        let querystring = create_querystring(&req);
        assert!(matches!(querystring, Err(_)));

        Ok(())
    }
}

#[cfg(test)]
mod create_request_url_tests {
    use super::*;
    #[test]
    fn test_create_url_with_since_date() -> Result<()> {
        let base_url = Url::parse("https://webmention.io")?;
        let date = DateTime::parse_from_str("2022-03-08T13:05:27-0100", "%FT%T%z")?;

        let req = GetWebmentionsRequest {
            since: Some(WebmentionsSince::SinceDate(date)),
            types: None,
        };

        let expected = "https://webmention.io/api/mentions.jf2?since=2022-03-08T13:05:27-0100";
        let api_url = create_request_url(&base_url, &req)?;
        assert_eq!(api_url.as_str(), expected);

        Ok(())
    }
}
