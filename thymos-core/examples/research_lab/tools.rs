//! Tools for research agents (browser, web search, etc.)

use thymos_core::error::{Result, ThymosError};
use serde_json::{json, Value};
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(feature = "browser-playwright")]
use playwright::Playwright;

/// Tool trait for agent capabilities
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, args: Value) -> Result<ToolResult>;
}

/// Result from tool execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub content: String,
    pub metadata: Value,
}

/// Browser tool for web research using Playwright
#[cfg(feature = "browser-playwright")]
pub struct BrowserTool {
    playwright: Arc<Playwright>,
}

#[cfg(feature = "browser-playwright")]
impl BrowserTool {
    pub async fn new() -> Result<Self> {
        let playwright = Playwright::initialize().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to initialize Playwright: {}", e))
        })?;
        
        playwright.prepare().map_err(|e| {
            ThymosError::Configuration(format!("Failed to prepare Playwright: {}", e))
        })?;
        
        Ok(Self {
            playwright: Arc::new(playwright),
        })
    }

    pub async fn fetch_url(&self, url: &str) -> Result<String> {
        let chromium = self.playwright.chromium();
        let browser = chromium.launcher()
            .headless(true)
            .launch()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to launch browser: {}", e))
            })?;
        
        let context = browser.context_builder().build().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to create browser context: {}", e))
        })?;
        
        let page = context.new_page().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to create page: {}", e))
        })?;
        
        page.goto_builder(url).goto().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to navigate to {}: {}", url, e))
        })?;
        
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        let content: String = page.eval("() => document.documentElement.outerHTML").await
            .map_err(|e| ThymosError::Configuration(format!("Failed to get page content: {}", e)))?;
        
        let text = self.extract_text(&content);
        
        browser.close().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to close browser: {}", e))
        })?;
        
        Ok(text)
    }

    fn extract_text(&self, html: &str) -> String {
        use regex::Regex;
        
        let re = Regex::new(r"<script[^>]*>.*?</script>").unwrap();
        let html = re.replace_all(html, "");
        
        let re = Regex::new(r"<style[^>]*>.*?</style>").unwrap();
        let html = re.replace_all(&html, "");
        
        let re = Regex::new(r"<nav[^>]*>.*?</nav>").unwrap();
        let html = re.replace_all(&html, "");
        
        let re = Regex::new(r"<header[^>]*>.*?</header>").unwrap();
        let html = re.replace_all(&html, "");
        
        let re = Regex::new(r"<footer[^>]*>.*?</footer>").unwrap();
        let html = re.replace_all(&html, "");
        
        let re = Regex::new(r"<[^>]+>").unwrap();
        let text = re.replace_all(&html, " ");
        
        let re = Regex::new(r"\s+").unwrap();
        let text = re.replace_all(&text, " ");
        
        text.trim().to_string()
    }
}

#[cfg(not(feature = "browser-playwright"))]
pub struct BrowserTool {
    client: reqwest::Client,
}

#[cfg(not(feature = "browser-playwright"))]
impl BrowserTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .unwrap(),
        }
    }

    pub async fn fetch_url(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ThymosError::Configuration(format!(
                "Failed to fetch URL {}: {}",
                url, e
            )))?;

        if !response.status().is_success() {
            return Err(ThymosError::Configuration(format!(
                "HTTP error {} for URL {}",
                response.status(),
                url
            )));
        }

        let text = response.text().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to read response: {}", e))
        })?;

        Ok(self.extract_text(&text))
    }

    fn extract_text(&self, html: &str) -> String {
        use regex::Regex;
        
        let re = Regex::new(r"<script[^>]*>.*?</script>").unwrap();
        let html = re.replace_all(html, "");
        
        let re = Regex::new(r"<style[^>]*>.*?</style>").unwrap();
        let html = re.replace_all(&html, "");
        
        let re = Regex::new(r"<[^>]+>").unwrap();
        let text = re.replace_all(&html, " ");
        
        let re = Regex::new(r"\s+").unwrap();
        let text = re.replace_all(&text, " ");
        
        text.trim().to_string()
    }
}

#[async_trait::async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser_fetch"
    }

    fn description(&self) -> &str {
        "Fetch and extract text content from a URL using a real browser"
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ThymosError::Configuration("Missing 'url' parameter".to_string())
            })?;

        let content = self.fetch_url(url).await?;

        Ok(ToolResult {
            success: true,
            content,
            metadata: json!({
                "url": url,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        })
    }
}

/// Web search tool using Playwright to search DuckDuckGo
#[cfg(feature = "browser-playwright")]
pub struct WebSearchTool {
    browser_tool: BrowserTool,
}

#[cfg(feature = "browser-playwright")]
impl WebSearchTool {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            browser_tool: BrowserTool::new().await?,
        })
    }

    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        let chromium = self.browser_tool.playwright.chromium();
        let browser = chromium.launcher()
            .headless(true)
            .launch()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to launch browser: {}", e))
            })?;
        
        let context = browser.context_builder().build().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to create browser context: {}", e))
        })?;
        
        let page = context.new_page().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to create page: {}", e))
        })?;
        
        // Use DuckDuckGo HTML version for more stable structure
        let search_url = format!("https://html.duckduckgo.com/html/?q={}", 
            urlencoding::encode(query));
        
        eprintln!("🔍 Navigating to: {}", search_url);
        page.goto_builder(&search_url).goto().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to navigate to search: {}", e))
        })?;
        
        // Wait for results to load - DuckDuckGo HTML can take a moment
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        
        // DuckDuckGo HTML version: result links are in <a class="result__a"> elements
        // Try the selector and verify we get results
        let result_links = page.query_selector_all("a.result__a").await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to query search results: {}", e))
        })?;
        
        eprintln!("📊 Found {} result links with selector 'a.result__a'", result_links.len());
        
        if result_links.is_empty() {
            // Try to get page HTML for debugging
            let page_html: String = page.eval("() => document.body.innerHTML").await
                .unwrap_or_else(|_| "Failed to get HTML".to_string());
            
            // Check if page loaded correctly
            let page_title: String = page.eval("() => document.title").await
                .unwrap_or_else(|_| "Unknown".to_string());
            
            return Err(ThymosError::Configuration(format!(
                "No search results found for query: '{}'. Page title: '{}'. \
                DuckDuckGo may have changed their HTML structure or blocked the request. \
                Found {} total links on page.",
                query, page_title,
                page.query_selector_all("a").await.unwrap_or_default().len()
            )));
        }
        
        let mut results = Vec::new();
        
        for link in result_links.iter().take(max_results) {
            let title = match link.text_content().await {
                Ok(Some(t)) => t.trim().to_string(),
                _ => continue,
            };
            
            if title.is_empty() {
                continue;
            }
            
            let href = match link.get_attribute("href").await {
                Ok(Some(h)) => h,
                _ => continue,
            };
            
            // Handle DuckDuckGo redirect URLs (/l/?kh=...&uddg=...)
            let url = if href.starts_with("/l/") {
                // Extract actual URL from DuckDuckGo redirect parameter
                if let Some(start) = href.find("uddg=") {
                    let url_part = if let Some(end) = href[start+5..].find("&") {
                        &href[start+5..start+5+end]
                    } else {
                        &href[start+5..]
                    };
                    match urlencoding::decode(url_part) {
                        Ok(decoded) => decoded.to_string(),
                        Err(_) => continue, // Skip URLs that can't be decoded
                    }
                } else {
                    continue; // Skip redirect URLs without uddg parameter
                }
            } else if href.starts_with("http://") || href.starts_with("https://") {
                href
            } else {
                continue;
            };
            
            // Skip DuckDuckGo internal links (but allow redirects with uddg=)
            if url.contains("duckduckgo.com") && !url.contains("uddg=") {
                continue;
            }
            
            results.push(SearchResult {
                title,
                url,
                snippet: String::new(),
            });
        }
        
        if results.is_empty() {
            return Err(ThymosError::Configuration(
                format!("Found {} result links but failed to extract valid URLs. Query: {}", result_links.len(), query)
            ));
        }
        
        browser.close().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to close browser: {}", e))
        })?;
        
        Ok(results)
    }
}

#[cfg(not(feature = "browser-playwright"))]
pub struct WebSearchTool {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(not(feature = "browser-playwright"))]
impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn search(&self, _query: &str, _max_results: usize) -> Result<Vec<SearchResult>> {
        Err(ThymosError::Configuration(
            "Web search requires browser-playwright feature".to_string()
        ))
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information using DuckDuckGo"
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ThymosError::Configuration("Missing 'query' parameter".to_string())
            })?;

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        let results = self.search(query, max_results).await?;

        let content = results
            .iter()
            .map(|r| format!("Title: {}\nURL: {}\n", r.title, r.url))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult {
            success: true,
            content,
            metadata: json!({
                "query": query,
                "result_count": results.len(),
                "results": results.iter().map(|r| json!({
                    "title": r.title,
                    "url": r.url,
                })).collect::<Vec<_>>(),
            }),
        })
    }
}
