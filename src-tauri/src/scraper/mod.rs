use anyhow::Result;
use crate::library::library_service::Manga;
use log::{info, warn};
use regex::Regex;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

pub struct WebScraper;

impl WebScraper {
    fn html_looks_blocked(html: &str) -> bool {
        let low = html.to_lowercase();
        low.contains("sorry, you have been blocked")
            || low.contains("attention required")
            || low.contains("cloudflare")
            || low.contains("cf-challenge")
            || low.contains("/cdn-cgi/")
    }

    pub async fn search_mangalivre_paginated(query: &str, page: u32) -> Result<(Vec<Manga>, u32)> {
        let page = page.max(1);
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let encoded_q = urlencoding::encode(query.trim());
        let candidate_urls: Vec<String> = if query.trim().is_empty() {
            if page == 1 {
                vec![
                    "https://mangalivre.tv/".to_string(),
                    "https://mangalivre.to/".to_string(),
                ]
            } else {
                vec![
                    format!("https://mangalivre.tv/page/{}/", page),
                    format!("https://mangalivre.to/page/{}/", page),
                ]
            }
        } else if page == 1 {
            vec![
                format!("https://mangalivre.tv/?s={}&post_type=wp-manga", encoded_q),
                format!("https://mangalivre.to/?s={}&post_type=wp-manga", encoded_q),
            ]
        } else {
            vec![
                format!("https://mangalivre.tv/page/{}/?s={}&post_type=wp-manga", page, encoded_q),
                format!("https://mangalivre.to/page/{}/?s={}&post_type=wp-manga", page, encoded_q),
            ]
        };

        let mut last_err: Option<String> = None;
        for url in candidate_urls {
            info!("MangaLivre scrape url={} query='{}' page={}", url, query, page);
            match client.get(&url).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        last_err = Some(format!("http status {} for {}", resp.status(), url));
                        continue;
                    }

                    let html = resp.text().await?;
                    if Self::html_looks_blocked(&html) {
                        last_err = Some(format!("blocked by anti-bot at {}", url));
                        continue;
                    }

                    let total_pages = Self::parse_total_pages(&html).max(page);
                    let items = Self::parse_mangalivre_cards(&html, query)?;

                    if !items.is_empty() {
                        return Ok((items, total_pages));
                    }

                    last_err = Some(format!("parsed 0 items for {}", url));
                }
                Err(e) => {
                    last_err = Some(format!("request error for {}: {}", url, e));
                }
            }
        }

        Err(anyhow::anyhow!(
            "MangaLivre scraping failed: {}",
            last_err.unwrap_or_else(|| "unknown error".to_string())
        ))
    }

    fn parse_total_pages(html: &str) -> u32 {
        // Handles links like /manga/page/44/ and /page/44/?s=...
        let re = Regex::new(r"/page/(\d+)/").unwrap();
        let mut max_page = 1u32;
        for cap in re.captures_iter(html) {
            if let Some(m) = cap.get(1) {
                if let Ok(v) = m.as_str().parse::<u32>() {
                    if v > max_page {
                        max_page = v;
                    }
                }
            }
        }
        max_page
    }

    fn parse_mangalivre_cards(html: &str, query: &str) -> Result<Vec<Manga>> {
        let doc = Html::parse_document(html);
        let card_selector = Selector::parse("li, article, .page-item-detail, .c-tabs-item, .row.c-tabs-item")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre card selector"))?;
        let list_selector = Selector::parse("ul.seriesList > li")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre series list selector"))?;
        let title_selector = Selector::parse(
            "h2 a[href*='/manga/'], h3 a[href*='/manga/'], .post-title a, .entry-title a, .page-item-detail a"
        )
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre title selector"))?;
        let generic_anchor_selector = Selector::parse("a[href*='/manga/'], a[title][href]")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre anchor selector"))?;
        let item_title_selector = Selector::parse("span.series-title, h2, h3")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre item title selector"))?;
        let item_anchor_selector = Selector::parse("a[href]")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre item anchor selector"))?;
        let item_cover_selector = Selector::parse(".cover-image, .series-cover, img[data-src], img[src], source[srcset]")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre item cover selector"))?;
        let item_desc_selector = Selector::parse(".series-desc")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre item description selector"))?;
        let item_chapters_selector = Selector::parse("span.series-chapters")
            .map_err(|_| anyhow::anyhow!("failed to parse MangaLivre item chapters selector"))?;

        // Prebuild href -> cover map so fallback title selectors can still show covers.
        let mut cover_by_href: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for card in doc.select(&card_selector) {
            let href = card
                .select(&item_anchor_selector)
                .filter_map(|a| a.value().attr("href"))
                .find(|h| h.contains("/manga/") && !h.contains("/capitulo-"));

            let abs_href = match href {
                Some(h) => Self::to_abs_mangalivre(h).ok(),
                None => None,
            };
            let abs_href = match abs_href {
                Some(v) => v,
                None => continue,
            };

            let cover = Self::extract_image_url_from_element(&card, &item_cover_selector)
                .and_then(|u| Self::to_abs_mangalivre(&u).ok().or(Some(u)));

            if let Some(c) = cover {
                cover_by_href.entry(abs_href).or_insert(c);
            }
        }

        // Extra fallback: map href -> image directly from raw HTML for layouts where
        // title/link and image are not wrapped in the same parsed card node.
        if let Ok(re_href_cover) = Regex::new(
            r#"(?is)<a[^>]*href=[\"'](?P<href>[^\"']*/manga/[^\"']*)[\"'][^>]*>.*?<img[^>]+(?:data-src|src)=[\"'](?P<cover>[^\"']+)[\"']"#,
        ) {
            for caps in re_href_cover.captures_iter(html) {
                let href = match caps.name("href") {
                    Some(v) => v.as_str(),
                    None => continue,
                };
                let cover = match caps.name("cover") {
                    Some(v) => v.as_str(),
                    None => continue,
                };

                let abs_href = match Self::to_abs_mangalivre(href) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let cover = Self::normalize_image_url(cover);
                let cover = Self::to_abs_mangalivre(&cover).unwrap_or(cover);

                cover_by_href.entry(abs_href).or_insert(cover);
            }
        }

        let mut seen = HashSet::new();
        let mut out: Vec<Manga> = Vec::new();
        let q = query.trim().to_lowercase();

        // Prefer MangaLivre native list layout (`ul.seriesList`) when present.
        for li in doc.select(&list_selector) {
            let href = li
                .select(&item_anchor_selector)
                .filter_map(|a| a.value().attr("href"))
                .find(|h| h.contains("/manga/"));
            let href = match href {
                Some(h) if !h.contains("/capitulo-") => h,
                _ => continue,
            };

            let abs = Self::to_abs_mangalivre(href)?;
            if seen.contains(&abs) {
                continue;
            }

            let title = li
                .select(&item_title_selector)
                .next()
                .map(|n| n.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    li.select(&item_anchor_selector)
                        .next()
                        .and_then(|a| a.value().attr("title"))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or_default();

            if title.is_empty() {
                continue;
            }
            if !q.is_empty() && !title.to_lowercase().contains(&q) {
                continue;
            }

            let cover = Self::extract_image_url_from_element(&li, &item_cover_selector)
                .and_then(|u| Self::to_abs_mangalivre(&u).ok().or(Some(u)))
                .or_else(|| cover_by_href.get(&abs).cloned());

            let synopsis = li
                .select(&item_desc_selector)
                .next()
                .map(|n| n.text().collect::<Vec<_>>().join(" ").replace('\n', " "))
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|s| !s.is_empty());

            let total_chapters = li
                .select(&item_chapters_selector)
                .next()
                .map(|n| n.text().collect::<Vec<_>>().join(" "))
                .and_then(|t| {
                    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
                    digits.parse::<i32>().ok()
                })
                .unwrap_or(0);

            seen.insert(abs.clone());
            out.push(Manga {
                id: abs,
                title,
                source_id: "mangalivre".to_string(),
                source_name: "Manga Livre".to_string(),
                cover_path: cover,
                synopsis,
                status: "ongoing".to_string(),
                rating: 0.0,
                language: "Português".to_string(),
                local_path: String::new(),
                total_chapters,
                downloaded_chapters: 0,
                last_updated: chrono::Local::now().to_rfc3339(),
            });
        }

        if !out.is_empty() {
            let covers = out
                .iter()
                .filter(|m| m.cover_path.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false))
                .count();
            info!("MangaLivre parsed {} results via seriesList ({} with cover)", out.len(), covers);
            return Ok(out);
        }

        // Prefer heading anchors first
        for a in doc.select(&title_selector) {
            if let Some(href) = a.value().attr("href") {
                if href.contains("/capitulo-") {
                    continue;
                }
                let abs = Self::to_abs_mangalivre(href)?;
                if seen.contains(&abs) {
                    continue;
                }
                let title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if title.is_empty() {
                    continue;
                }
                if !q.is_empty() && !title.to_lowercase().contains(&q)
                {
                    continue;
                }
                seen.insert(abs.clone());
                let cover = cover_by_href.get(&abs).cloned();
                out.push(Self::mk_manga(&abs, &title, cover));
            }
        }

        // Fallback when heading selector misses items
        if out.is_empty() {
            for a in doc.select(&generic_anchor_selector) {
                if let Some(href) = a.value().attr("href") {
                    if href.contains("/capitulo-") {
                        continue;
                    }
                    let abs = Self::to_abs_mangalivre(href)?;
                    if seen.contains(&abs) {
                        continue;
                    }

                    let mut title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if title.is_empty() {
                        title = a.value().attr("title").unwrap_or("").trim().to_string();
                    }
                    if title.is_empty() {
                        continue;
                    }
                    if !q.is_empty() && !title.to_lowercase().contains(&q)
                    {
                        continue;
                    }

                    seen.insert(abs.clone());
                    let cover = cover_by_href.get(&abs).cloned();
                    out.push(Self::mk_manga(&abs, &title, cover));
                }
            }
        }

        let covers = out
            .iter()
            .filter(|m| m.cover_path.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false))
            .count();
        info!("MangaLivre parsed {} results ({} with cover)", out.len(), covers);
        Ok(out)
    }

    fn extract_background_image_url(style: &str) -> Option<String> {
        let low = style.to_lowercase();
        let pos = low.find("url(")?;
        let after = &style[pos + 4..];
        let end = after.find(')')?;
        let mut val = after[..end].trim().to_string();
        val = val.trim_matches('"').trim_matches('\'').to_string();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    }

    fn extract_image_url_from_element(el: &ElementRef<'_>, image_selector: &Selector) -> Option<String> {
        // First try image-like tags/attributes.
        if let Some(raw) = el
            .select(image_selector)
            .find_map(|n| {
                for key in [
                    "data-src",
                    "data-lazy-src",
                    "data-original",
                    "data-srcset",
                    "data-lazy-srcset",
                    "srcset",
                    "src",
                    "style",
                ] {
                    if let Some(v) = n.value().attr(key) {
                        let t = v.trim();
                        if !t.is_empty() {
                            return Some(t.to_string());
                        }
                    }
                }
                None
            })
        {
            if raw.contains("url(") {
                if let Some(v) = Self::extract_background_image_url(&raw) {
                    return Some(Self::normalize_image_url(&v));
                }
            }

            let from_srcset = raw
                .split(',')
                .next()
                .unwrap_or(&raw)
                .split_whitespace()
                .next()
                .unwrap_or(&raw)
                .trim();

            if !from_srcset.is_empty() {
                return Some(Self::normalize_image_url(from_srcset));
            }
        }

        // Fallback: some cards keep URL in inline style on the container itself.
        if let Some(style) = el.value().attr("style") {
            if let Some(v) = Self::extract_background_image_url(style) {
                return Some(Self::normalize_image_url(&v));
            }
        }

        None
    }

    fn normalize_image_url(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("//") {
            format!("https:{}", trimmed)
        } else {
            trimmed.to_string()
        }
    }

    fn to_abs_niadd(href: &str) -> Result<String> {
        if href.starts_with("http://") || href.starts_with("https://") {
            return Ok(href.to_string());
        }
        let base = Url::parse("https://br.niadd.com")?;
        Ok(base.join(href)?.to_string())
    }

    fn mk_manga(url: &str, title: &str, cover: Option<String>) -> Manga {
        Manga {
            id: url.to_string(), // keep source URL as stable id for details/download
            title: title.to_string(),
            source_id: "mangalivre".to_string(),
            source_name: "Manga Livre".to_string(),
            cover_path: cover,
            synopsis: None,
            status: "ongoing".to_string(),
            rating: 0.0,
            language: "Português".to_string(),
            local_path: String::new(),
            total_chapters: 0,
            downloaded_chapters: 0,
            last_updated: chrono::Local::now().to_rfc3339(),
        }
    }

    fn to_abs_mangalivre(href: &str) -> Result<String> {
        if href.starts_with("http://") || href.starts_with("https://") {
            return Ok(href.to_string());
        }
        let base = Url::parse("https://mangalivre.tv")?;
        Ok(base.join(href)?.to_string())
    }

    pub async fn search_manga_livre(query: &str) -> Result<Vec<Manga>> {
        let (items, _) = Self::search_mangalivre_paginated(query, 1).await?;
        Ok(items)
    }

    pub async fn search_niadd(query: &str) -> Result<Vec<Manga>> {
        info!("Scraping Niadd for query: '{}'", query);

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(12))
            .build()?;

        let encoded_q = urlencoding::encode(query.trim());
        let candidate_urls: Vec<String> = if query.trim().is_empty() {
            vec![
                "https://br.niadd.com/list/Hot-Manga/".to_string(),
                "https://br.niadd.com/list/New-Update/".to_string(),
                "https://br.niadd.com/".to_string(),
            ]
        } else {
            vec![
                format!("https://br.niadd.com/search/?search_type=1&name={}", encoded_q),
                format!("https://br.niadd.com/search/?name={}", encoded_q),
            ]
        };

        let mut seen = HashSet::new();
        let mut merged: Vec<Manga> = Vec::new();
        let mut last_err: Option<String> = None;

        for url in candidate_urls {
            match client.get(&url).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        last_err = Some(format!("http status {} for {}", resp.status(), url));
                        continue;
                    }

                    let html = resp.text().await?;
                    let parsed = Self::parse_niadd_cards(&html, query)?;
                    if parsed.is_empty() {
                        continue;
                    }

                    for m in parsed {
                        if seen.insert(m.id.clone()) {
                            merged.push(m);
                        }
                    }
                }
                Err(e) => {
                    last_err = Some(format!("request error for {}: {}", url, e));
                }
            }
        }

        if merged.is_empty() {
            if let Some(e) = last_err {
                warn!("Niadd scraping returned no items: {}", e);
            }
            return Ok(Vec::new());
        }

        Ok(merged.into_iter().take(80).collect())
    }

    fn parse_niadd_cards(html: &str, query: &str) -> Result<Vec<Manga>> {
        let doc = Html::parse_document(html);
        let title_selector = Selector::parse("a[href*='/manga/']")
            .map_err(|_| anyhow::anyhow!("failed to parse Niadd anchor selector"))?;
        let card_selector = Selector::parse("li, article, .book-item, .manga-item, .list-item, .media")
            .map_err(|_| anyhow::anyhow!("failed to parse Niadd card selector"))?;
        let card_anchor_selector = Selector::parse("a[href]")
            .map_err(|_| anyhow::anyhow!("failed to parse Niadd card anchor selector"))?;
        let card_cover_selector = Selector::parse("img[data-src], img[src], source[srcset], [style]")
            .map_err(|_| anyhow::anyhow!("failed to parse Niadd card cover selector"))?;

        let mut cover_by_href: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        // Primary fallback for Niadd hot/search pages where cover is under .manga-img.
        if let Ok(re_niadd_cover) = Regex::new(
            r#"(?is)<a[^>]*href=[\"'](?P<href>(?:https?://br\.niadd\.com)?/manga/[^\"']+|https?://br\.niadd\.com/manga/[^\"']+)[\"'][^>]*>\s*<div[^>]*class=[\"'][^\"']*manga-img[^\"']*[\"'][^>]*>\s*<img[^>]+(?:data-src|src)=[\"'](?P<cover>[^\"']+)[\"']"#,
        ) {
            for caps in re_niadd_cover.captures_iter(html) {
                let href = match caps.name("href") {
                    Some(v) => v.as_str(),
                    None => continue,
                };
                let cover = match caps.name("cover") {
                    Some(v) => v.as_str(),
                    None => continue,
                };

                let abs_href = match Self::to_abs_niadd(href) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let cover = Self::normalize_image_url(cover);
                let cover = Self::to_abs_niadd(&cover).unwrap_or(cover);

                cover_by_href.entry(abs_href).or_insert(cover);
            }
        }

        for card in doc.select(&card_selector) {
            let href = card
                .select(&card_anchor_selector)
                .filter_map(|a| a.value().attr("href"))
                .find(|h| h.contains("/manga/") && !h.contains("/capitulo-"));
            let abs_href = match href {
                Some(h) => Self::to_abs_niadd(h).ok(),
                None => None,
            };
            let abs_href = match abs_href {
                Some(v) => v,
                None => continue,
            };

            let cover = Self::extract_image_url_from_element(&card, &card_cover_selector)
                .and_then(|u| Self::to_abs_niadd(&u).ok().or(Some(u)));
            if let Some(c) = cover {
                cover_by_href.entry(abs_href).or_insert(c);
            }
        }

        let mut seen = HashSet::new();
        let mut out: Vec<Manga> = Vec::new();
        let q = query.trim().to_lowercase();

        for a in doc.select(&title_selector) {
            let href = match a.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            if href.contains("/manga/.html") {
                continue;
            }

            let abs = Self::to_abs_niadd(href)?;

            if seen.contains(&abs) {
                continue;
            }

            let mut title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if title.is_empty() {
                title = a.value().attr("title").unwrap_or("").trim().to_string();
            }
            if title.is_empty() {
                continue;
            }

            if !q.is_empty() && !title.to_lowercase().contains(&q) {
                continue;
            }

            seen.insert(abs.clone());
            out.push(Manga {
                id: abs.clone(),
                title,
                source_id: "niadd".to_string(),
                source_name: "Niadd".to_string(),
                cover_path: cover_by_href.get(&abs).cloned(),
                synopsis: None,
                status: "ongoing".to_string(),
                rating: 0.0,
                language: "Português".to_string(),
                local_path: String::new(),
                total_chapters: 0,
                downloaded_chapters: 0,
                last_updated: chrono::Local::now().to_rfc3339(),
            });
        }

        Ok(out)
    }

    pub async fn search_source(source_id: &str, query: &str) -> Result<Vec<Manga>> {
        match source_id {
            "mangalivre" => Self::search_manga_livre(query).await,
            "niadd" => Self::search_niadd(query).await,
            _ => Ok(Vec::new()),
        }
    }

    pub async fn search_source_light(source_id: &str, query: &str) -> Result<Vec<Manga>> {
        match source_id {
            "mangalivre" => {
                let full = Self::search_manga_livre(query).await.unwrap_or_default();
                Ok(full.into_iter().take(60).collect())
            }
            "niadd" => {
                let full = Self::search_niadd(query).await.unwrap_or_default();
                Ok(full.into_iter().take(60).collect())
            }
            _ => Ok(Vec::new()),
        }
    }

    pub async fn get_manga_details(source_id: &str, manga_id: &str) -> Result<Option<Manga>> {
        if source_id == "mangalivre" {
            let url = if manga_id.starts_with("http://") || manga_id.starts_with("https://") {
                manga_id.to_string()
            } else {
                format!("https://mangalivre.tv/manga/{}/", manga_id.trim_matches('/'))
            };

            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(std::time::Duration::from_secs(20))
                .build()?;
            let html = client.get(&url).send().await?.text().await?;
            if Self::html_looks_blocked(&html) {
                return Err(anyhow::anyhow!(
                    "MangaLivre blocked the details request (Cloudflare/anti-bot)"
                ));
            }
            let doc = Html::parse_document(&html);

            let title = Selector::parse("h1, h2")
                .ok()
                .and_then(|s| doc.select(&s).next())
                .map(|n| n.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Manga".to_string());

            if title.to_lowercase().contains("sorry, you have been blocked") {
                return Err(anyhow::anyhow!(
                    "MangaLivre blocked the details page (title indicates anti-bot block)"
                ));
            }

            let total_chapters = Selector::parse("a[href*='/capitulo-']")
                .ok()
                .map(|s| doc.select(&s).count() as i32)
                .unwrap_or(0);

            let cover_path = Selector::parse(".cover-image, .series-cover, .manga-image picture img, .series-thumb img, img[src]")
                .ok()
                .and_then(|s| doc.select(&s).next())
                .and_then(|n| {
                    n.value()
                        .attr("data-src")
                        .or_else(|| n.value().attr("src"))
                        .or_else(|| n.value().attr("style"))
                })
                .and_then(|raw| {
                    if raw.contains("url(") {
                        Self::extract_background_image_url(raw)
                    } else {
                        Some(raw.to_string())
                    }
                })
                .and_then(|u| Self::to_abs_mangalivre(&u).ok());

            let synopsis = Selector::parse(".summary__content, .description-summary, p")
                .ok()
                .and_then(|s| doc.select(&s).next())
                .map(|n| n.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| s.len() > 20);

            return Ok(Some(Manga {
                id: url,
                title,
                source_id: "mangalivre".to_string(),
                source_name: "Manga Livre".to_string(),
                cover_path,
                synopsis,
                status: "ongoing".to_string(),
                rating: 0.0,
                language: "Português".to_string(),
                local_path: String::new(),
                total_chapters,
                downloaded_chapters: 0,
                last_updated: chrono::Local::now().to_rfc3339(),
            }));
        }

        if let Ok(list) = Self::search_source(source_id, "").await {
            for m in list {
                if m.id == manga_id {
                    return Ok(Some(m));
                }
            }
        }

        Ok(None)
    }
}
