// 新闻信息流模块：独立窗口创建 + RSS/Atom 抓取解析 + 本地缓存 + command 接口
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder};

use crate::storage::NewsFeed;

// ===== 数据结构（与前端 src/types.ts 对应） =====

/// 单条新闻
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: String,        // 直接使用 link（天然唯一，供前端 :key 与去重）
    pub title: String,
    pub summary: String,   // 已剥 HTML 的纯文本摘要
    pub link: String,
    pub source: String,    // 源名（来自 NewsFeed.name）
    pub published_at: i64, // unix 秒；解析失败为 0（排序时沉底）
}

/// 抓取结果缓存（news_cache.json 的结构）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewsCache {
    pub items: Vec<NewsItem>,
    pub fetched_at: Option<i64>,
}

// ===== 窗口创建（与便签同形态：无边框透明小窗，紧贴便签左侧） =====

/// 新闻窗口的逻辑尺寸与间距
const NEWS_WIDTH: f64 = 420.0;
const NEWS_HEIGHT: f64 = 640.0;
const NEWS_GAP: f64 = 8.0;

/// 新闻窗口位置：便签默认位置左侧、顶部对齐（屏幕过窄时兜底不越出左边界）
fn news_position(app: &tauri::AppHandle) -> (f64, f64) {
    let (sx, sy) = crate::sticky::sticky_position(app);
    let x = (sx - NEWS_WIDTH - NEWS_GAP).max(20.0);
    (x, sy)
}

/// 创建新闻窗口（启动时调用，创建后立即显示）
pub fn create_news_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let (x, y) = news_position(app);
    let window = WebviewWindowBuilder::new(app, "news", WebviewUrl::App("news.html".into()))
        .title("资讯")
        .decorations(false)
        .transparent(true)
        .always_on_top(false)
        .skip_taskbar(true)
        .resizable(false)
        .inner_size(NEWS_WIDTH, NEWS_HEIGHT)
        .position(x, y)
        .build()?;
    let _ = window.show();
    Ok(())
}

// ===== RSS / Atom 解析 =====

/// 流式解析器：RSS 2.0 <item> 与 Atom <entry> 双兼容
struct FeedParser<'a> {
    reader: Reader<&'a [u8]>,
    source: String,
}

impl<'a> FeedParser<'a> {
    fn new(xml: &'a str, source: &str) -> Self {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        Self {
            reader,
            source: source.to_string(),
        }
    }

    /// 扫描顶层事件，遇到 <item> / <entry> 起始标签交给 parse_item
    fn parse_items(&mut self) -> Vec<NewsItem> {
        let mut items = Vec::new();
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = e.local_name().as_ref().to_vec();
                    if name == b"item" || name == b"entry" {
                        let item = self.parse_item(&name);
                        // link 为空的条目无法跳转，直接丢弃
                        if !item.link.is_empty() {
                            items.push(item);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }
        items
    }

    /// 消费一个 item/entry 内的全部事件直到配对 End，组装 NewsItem
    fn parse_item(&mut self, tag: &[u8]) -> NewsItem {
        let mut item = NewsItem {
            id: String::new(),
            title: String::new(),
            summary: String::new(),
            link: String::new(),
            source: self.source.clone(),
            published_at: 0,
        };
        let mut title_raw = String::new();
        let mut summary_raw = String::new();
        let mut time_raw = String::new();
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = e.local_name().as_ref().to_vec();
                    match name.as_slice() {
                        // RSS: <link>文本</link>；Atom: <link href="...">（元素体为空）
                        b"link" => {
                            let href = Self::attr_href(&e);
                            let text = Self::text_until_end(&mut self.reader);
                            if !href.is_empty() {
                                if item.link.is_empty() && Self::is_alternate(&e) {
                                    item.link = href;
                                }
                            } else if !text.is_empty() && item.link.is_empty() {
                                item.link = text;
                            }
                        }
                        b"title" => title_raw = Self::text_until_end(&mut self.reader),
                        b"description" | b"summary" => {
                            summary_raw = Self::text_until_end(&mut self.reader)
                        }
                        b"pubDate" | b"published" | b"updated" | b"dc:date" => {
                            time_raw = Self::text_until_end(&mut self.reader)
                        }
                        _ => {}
                    }
                }
                // Atom 自闭合链接 <link href="..." />
                Ok(Event::Empty(e)) => {
                    if e.local_name().as_ref() == b"link" {
                        let href = Self::attr_href(&e);
                        if !href.is_empty() && item.link.is_empty() && Self::is_alternate(&e) {
                            item.link = href;
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    if e.local_name().as_ref() == tag {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }
        item.id = item.link.clone();
        item.title = HtmlText::strip(&title_raw);
        item.summary = HtmlText::strip(&summary_raw);
        item.published_at = parse_timestamp(&time_raw);
        item
    }

    /// 读取当前元素内部的全部文本（兼容 CDATA 与转义），直到配对 End
    fn text_until_end(reader: &mut Reader<&'a [u8]>) -> String {
        let mut out = String::new();
        let mut depth = 0usize;
        loop {
            match reader.read_event() {
                Ok(Event::Text(t)) => {
                    if let Ok(s) = t.decode() {
                        out.push_str(&s);
                    }
                }
                Ok(Event::CData(t)) => {
                    out.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
                Ok(Event::Start(_)) => depth += 1,
                Ok(Event::End(_)) => {
                    if depth == 0 {
                        return out;
                    }
                    depth -= 1;
                }
                Ok(Event::Eof) => return out,
                Err(_) => return out,
                _ => {}
            }
        }
    }

    /// 提取元素的 href 属性（Atom 链接）
    fn attr_href(e: &quick_xml::events::BytesStart) -> String {
        for attr in e.attributes().flatten() {
            if attr.key.as_ref() == b"href" {
                let raw = String::from_utf8_lossy(&attr.value);
                if let Ok(s) = quick_xml::escape::unescape(&raw) {
                    return s.into_owned();
                }
            }
        }
        String::new()
    }

    /// Atom 链接是否为正文链接（无 rel 或 rel="alternate"；rel="self" 指向 feed 自身，需排除）
    fn is_alternate(e: &quick_xml::events::BytesStart) -> bool {
        for attr in e.attributes().flatten() {
            if attr.key.as_ref() == b"rel" {
                let raw = String::from_utf8_lossy(&attr.value);
                if let Ok(s) = quick_xml::escape::unescape(&raw) {
                    return s == "alternate";
                }
            }
        }
        true
    }
}

/// 时间解析：RSS pubDate(RFC2822) / Atom published·updated(RFC3339) → unix 秒，失败回退 0
fn parse_timestamp(s: &str) -> i64 {
    let s = s.trim();
    chrono::DateTime::parse_from_rfc2822(s)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s))
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

// ===== HTML 转纯文本 =====

/// HTML 清理器：剥标签 + 解码实体 + 折叠空白 + 截断
/// （不用 quick_xml::unescape：&nbsp; 等非 XML 预定义实体会报错）
struct HtmlText;

impl HtmlText {
    /// 上限（字符数），超出截断加省略号
    const MAX_LEN: usize = 200;

    fn strip(html: &str) -> String {
        // 1. 剥标签（标签位置替换为空格，避免词汇粘连）
        let mut no_tags = String::with_capacity(html.len());
        let mut in_tag = false;
        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    no_tags.push(' ');
                }
                c if !in_tag => no_tags.push(c),
                _ => {}
            }
        }
        // 2. 解码实体（循环两层，处理 &amp;lt; 这类双重转义）
        let mut s = Self::decode_entities(&no_tags);
        for _ in 0..2 {
            let next = Self::decode_entities(&s);
            if next == s {
                break;
            }
            s = next;
        }
        // 3. 折叠空白 + trim
        let mut out = String::with_capacity(s.len());
        let mut last_ws = false;
        for ch in s.trim().chars() {
            if ch.is_whitespace() {
                if !last_ws {
                    out.push(' ');
                }
                last_ws = true;
            } else {
                out.push(ch);
                last_ws = false;
            }
        }
        // 4. 截断
        if out.chars().count() > Self::MAX_LEN {
            let head: String = out.chars().take(Self::MAX_LEN).collect();
            format!("{head}…")
        } else {
            out
        }
    }

    /// 解码常见 HTML 实体（命名 + 十进制/十六进制数字实体）
    fn decode_entities(s: &str) -> String {
        if !s.contains('&') {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(s.len());
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] == '&' {
                // 向后找 10 个字符内的分号
                if let Some(semi) = chars[i + 1..].iter().take(10).position(|&c| c == ';') {
                    let ent: String = chars[i + 1..i + 1 + semi].iter().collect();
                    let decoded = match ent.as_str() {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        "nbsp" => Some(' '),
                        _ => Self::decode_numeric(&ent),
                    };
                    if let Some(c) = decoded {
                        out.push(c);
                        i += semi + 2;
                        continue;
                    }
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    /// 数字实体 &#123; / &#x1F600; → 字符
    fn decode_numeric(ent: &str) -> Option<char> {
        let num = ent.strip_prefix('#')?;
        let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
            u32::from_str_radix(hex, 16).ok()
        } else {
            num.parse::<u32>().ok()
        };
        code.and_then(char::from_u32)
    }
}

// ===== 抓取 =====

/// 单源条数上限与合并后总上限
const PER_FEED_LIMIT: usize = 20;
const TOTAL_LIMIT: usize = 60;
/// 单次请求整体超时（连接 + 读）
const FETCH_TIMEOUT_SECS: u64 = 15;

/// 抓取器：ureq 阻塞客户端（跑在 spawn_blocking 中），单源失败静默返回空不影响其他源
struct NewsFetcher {
    agent: ureq::Agent,
}

impl NewsFetcher {
    fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS))
            // 部分 RSS 服务拒绝空 UA
            .user_agent("Liaos/0.1")
            .build();
        Self { agent }
    }

    fn fetch_feed(&self, feed: &NewsFeed) -> Vec<NewsItem> {
        let body = match self.agent.get(&feed.url).call() {
            Ok(resp) => resp.into_string().unwrap_or_default(),
            Err(_) => return Vec::new(),
        };
        let mut items = FeedParser::new(&body, &feed.name).parse_items();
        items.truncate(PER_FEED_LIMIT);
        items
    }
}

// ===== 后台刷新（fire-and-forget，同 maybe_auto_sync 范式） =====

/// 防重入标志：抓取进行中时忽略新的触发
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// 复位标志的守卫（任何路径退出都复位，含提前 return）
struct InFlightGuard;

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

/// 触发一次后台抓取：并发抓所有启用源 → 合并排序去重 → 写缓存 → emit("news-updated")
pub fn spawn_refresh(app: tauri::AppHandle) {
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        // 已在抓取：忽略本次触发，进行中的抓取完成后事件会携带最新结果
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _guard = InFlightGuard;

        let feeds: Vec<NewsFeed> = crate::storage::load_settings(&app)
            .map(|s| s.news_feeds.into_iter().filter(|f| f.enabled).collect())
            .unwrap_or_default();
        if feeds.is_empty() {
            let _ = app.emit("news-updated", read_cache(&app));
            return;
        }

        // 每源一个阻塞任务并发抓取（最坏 15s 收敛，而非串行 3×15s）
        let fetcher = NewsFetcher::new();
        let handles: Vec<_> = feeds
            .into_iter()
            .map(|feed| {
                let agent = fetcher.agent.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    NewsFetcher { agent }.fetch_feed(&feed)
                })
            })
            .collect();
        let mut items = Vec::new();
        for h in handles {
            if let Ok(list) = h.await {
                items.extend(list);
            }
        }

        items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        let mut seen = HashSet::new();
        items.retain(|i| seen.insert(i.id.clone()));
        items.truncate(TOTAL_LIMIT);

        // 全部失败时保留旧缓存，只推送现状
        if !items.is_empty() {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let _ = write_cache(&app, &NewsCache { items, fetched_at: Some(now_secs) });
        }
        let _ = app.emit("news-updated", read_cache(&app));
    });
}

// ===== 缓存读写（复用 storage.rs 泛型函数） =====

fn cache_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("news_cache.json")
}

/// 读取缓存（无文件/损坏时返回空缓存）
pub fn read_cache(app: &tauri::AppHandle) -> NewsCache {
    crate::storage::data_dir(app)
        .map(|d| crate::storage::read_json(&cache_path(&d), NewsCache::default()))
        .unwrap_or_default()
}

fn write_cache(app: &tauri::AppHandle, cache: &NewsCache) -> Result<(), String> {
    let dir = crate::storage::data_dir(app)?;
    crate::storage::write_json(&cache_path(&dir), cache)
}

// ===== Commands =====

/// 立即返回本地缓存（窗口打开时调用，可能是空/旧数据）
#[tauri::command]
pub fn get_news(app: tauri::AppHandle) -> Result<NewsCache, String> {
    Ok(read_cache(&app))
}

/// 触发一次后台抓取（立即返回，结果经 "news-updated" 事件推送）
#[tauri::command]
pub fn refresh_news(app: tauri::AppHandle) -> Result<(), String> {
    spawn_refresh(app);
    Ok(())
}
