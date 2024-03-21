use crate::gemfeed::GemfeedEntry;
use anyhow::Result;

pub fn strip_before(entry: &mut GemfeedEntry, marker: &str) -> Result<()> {
    let body = entry.body_mut()?;
    let sanitized_body = match body.find(marker) {
        Some(index) => body.split_at(index + marker.len()).1,
        _ => &body,
    };

    *body = sanitized_body.to_owned();
    Ok(())
}


pub fn strip_after(entry: &mut GemfeedEntry, marker: &str) -> Result<()> {
    let body = entry.body_mut()?;
    let sanitized_body = match body.rfind(marker) {
        Some(index) => body.split_at(index).0,
        _ => &body,
    };

    *body = sanitized_body.to_owned();
    Ok(())
}
