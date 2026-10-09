//! Find files on a page (B9.3): the page is read once, and the files it links
//! to are listed for the window to pick from. Nothing is downloaded here.

use serde::Serialize;

use super::{Service, UiError};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FoundFile {
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageFiles {
    /// The page's title, for naming the group.
    pub title: Option<String>,
    pub files: Vec<FoundFile>,
}

impl Service {
    pub async fn files_on_page(&self, link: &str) -> Result<PageFiles, UiError> {
        let link = link.trim();
        fuselane_core::runner::parse_link(link).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        let (place, html) = fuselane_core::runner::fetch_page(link)
            .await
            .map_err(|m| UiError::new("page-failed", m, None))?;
        let files: Vec<FoundFile> = fuselane_core::grab::files_on_page(&html, &place)
            .into_iter()
            .map(|f| FoundFile {
                url: f.url,
                name: f.name,
            })
            .collect();
        if files.is_empty() {
            return Err(UiError::new(
                "no-files",
                "That page doesn't link to any files.",
                Some("Paste the link of the file itself, or a page that lists downloads."),
            ));
        }
        Ok(PageFiles {
            title: fuselane_core::grab::title(&html),
            files,
        })
    }
}
