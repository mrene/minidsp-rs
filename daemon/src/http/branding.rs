//! Branding for the web interface (name, logo, accent color), shared by every client.
//! Stored in `branding.json`, next to the preset library.
use std::path::PathBuf;

use anyhow::Context;
use hyper::{Body, Request, Response};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    helpers::{parse_body, serialize_response},
    presets::presets_dir,
    Error,
};

/// Font pairs the web interface bundles; "system" uses the operating system's fonts
pub const FONTS: [&str; 5] = ["system", "plex", "inter", "barlow", "space"];

/// Largest accepted logo, as a data URL (about 500 KB of image data)
const MAX_LOGO_LEN: usize = 700_000;
const MAX_TITLE_LEN: usize = 40;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Branding {
    /// Name shown in the header and the browser tab; the default name is used when empty
    #[serde(default)]
    pub title: String,

    /// Logo as a data URL (`data:image/png;base64,…`); PNG, JPEG, WebP or SVG
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,

    /// Accent color as `#rrggbb`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,

    /// Font pair: system, plex, inter, barlow or space
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

async fn branding_file() -> PathBuf {
    let presets = presets_dir().await;
    presets
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(presets)
        .join("branding.json")
}

fn bad_request(reason: impl Into<String>) -> Error {
    Error::ParseError {
        reason: reason.into(),
    }
}

fn validate(mut branding: Branding) -> Result<Branding, Error> {
    branding.title = branding.title.trim().to_string();
    if branding.title.chars().count() > MAX_TITLE_LEN {
        return Err(bad_request(format!(
            "the name can be at most {MAX_TITLE_LEN} characters"
        )));
    }
    if branding.title.chars().any(char::is_control) {
        return Err(bad_request("the name can't contain control characters"));
    }

    if let Some(logo) = &branding.logo {
        const TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/svg+xml"];
        let valid_type = TYPES
            .iter()
            .any(|t| logo.starts_with(&format!("data:{t};base64,")));
        if !valid_type {
            return Err(bad_request(
                "the logo must be a PNG, JPEG, WebP or SVG image",
            ));
        }
        if logo.len() > MAX_LOGO_LEN {
            return Err(bad_request(
                "the logo is too large; use an image under 500 KB",
            ));
        }
    }

    if let Some(accent) = &branding.accent {
        let valid = accent.len() == 7
            && accent.starts_with('#')
            && accent[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err(bad_request("the accent color must look like #1a2b3c"));
        }
        branding.accent = Some(accent.to_lowercase());
    }

    if let Some(font) = &branding.font {
        if !FONTS.contains(&font.as_str()) {
            return Err(bad_request(format!(
                "unknown font '{font}', use one of: {}",
                FONTS.join(", ")
            )));
        }
    }
    Ok(branding)
}

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let path = branding_file().await;
    let branding: Branding = match tokio::fs::read(&path).await {
        Ok(data) => serde_json::from_slice(&data)
            .with_context(|| format!("{} is not valid", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Branding::default(),
        Err(e) => {
            return Err(anyhow::Error::from(e)
                .context(format!("cannot read {}", path.display()))
                .into())
        }
    };
    Ok(serialize_response(&req, branding)?)
}

pub async fn put(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let branding = validate(parse_body(&mut req).await?)?;
    let path = branding_file().await;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(&branding).context("cannot serialize branding")?;
    tokio::fs::write(&tmp, data)
        .await
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(serialize_response(&req, branding)?)
}

#[cfg(test)]
mod test {
    use super::*;

    fn branding(title: &str, logo: Option<&str>, accent: Option<&str>) -> Branding {
        Branding {
            title: title.into(),
            logo: logo.map(Into::into),
            accent: accent.map(Into::into),
            font: None,
        }
    }

    #[test]
    fn validates_fonts() {
        let with_font = |f: &str| Branding {
            font: Some(f.into()),
            ..branding("", None, None)
        };
        assert!(validate(with_font("plex")).is_ok());
        assert!(validate(with_font("system")).is_ok());
        assert!(validate(with_font("comic-sans")).is_err());
    }

    #[test]
    fn validates_branding() {
        let ok = validate(branding(
            "  Corey's DSP ",
            Some("data:image/png;base64,AAAA"),
            Some("#1A2B3C"),
        ))
        .unwrap();
        assert_eq!(ok.title, "Corey's DSP");
        assert_eq!(ok.accent.as_deref(), Some("#1a2b3c"));
        assert!(validate(branding(
            "",
            Some("data:image/svg+xml;base64,PHN2Zz4="),
            None
        ))
        .is_ok());

        assert!(validate(branding(&"x".repeat(41), None, None)).is_err());
        assert!(validate(branding("", Some("data:text/html;base64,AAAA"), None)).is_err());
        assert!(validate(branding("", Some("https://example.com/logo.png"), None)).is_err());
        assert!(validate(branding("", None, Some("red"))).is_err());
        assert!(validate(branding("", None, Some("#12345g"))).is_err());
        let huge = format!("data:image/png;base64,{}", "A".repeat(MAX_LOGO_LEN));
        assert!(validate(branding("", Some(&huge), None)).is_err());
    }
}
