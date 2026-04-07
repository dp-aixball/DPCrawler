"""
Main crawler module for DPCrawler
"""
import os
import sys
import json
import time
import threading
from collections import deque
from concurrent.futures import ThreadPoolExecutor, as_completed
from urllib.parse import urljoin, urlparse
from typing import Set, Optional
import re

import requests
from bs4 import BeautifulSoup
try:
    import html2text
    HTML2TEXT_AVAILABLE = True
except ImportError:
    HTML2TEXT_AVAILABLE = False

from config import CrawlerConfig
from storage import StorageManager, CrawlResult
from dataclasses import asdict


class WebCrawler:
    """Web crawler for RAG knowledge collection"""

    MAX_PAGES = 200  # Maximum pages per crawl session

    CONCURRENT_WORKERS = 8  # Parallel crawl workers

    def __init__(self, config: CrawlerConfig):
        self.config = config
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": "Mozilla/5.0 (compatible; DPCrawler/1.0; +https://github.com/dpcrawler)"
        })
        self.visited_urls: Set[str] = set()
        self.new_files: list = []
        self.updated_files: list = []
        self.unchanged_count: int = 0
        self.error_count: int = 0
        self.page_count: int = 0
        self.storage: Optional[StorageManager] = None
        self.current_subdir: str = ""
        self.base_urls: list = []  # base URLs for scope checking
        self.bfs_queue: deque = deque()  # BFS queue (thread-safe with lock)
        self.url_depths: dict = {}  # Track depth of each URL
        self._lock = threading.Lock()  # Protect shared state

    @staticmethod
    def _normalize_url(url: str) -> str:
        """Normalize URL: remove fragment, sort query params, strip trailing slash"""
        parsed = urlparse(url)
        # Remove fragment
        path = parsed.path.rstrip('/') or '/'
        # Keep query but drop fragment
        normalized = f"{parsed.scheme}://{parsed.netloc}{path}"
        if parsed.query:
            normalized += f"?{parsed.query}"
        return normalized

    def _is_in_scope(self, url: str) -> bool:
        """Check if URL is within the scope of any base URL (same domain + path prefix)"""
        parsed = urlparse(url)
        url_path = parsed.path or '/'
        for base in self.base_urls:
            bp = urlparse(base)
            base_path = bp.path.rstrip('/') or '/'
            # Normalize: /foo should match /foo and /foo/bar but not /foobar
            if parsed.netloc == bp.netloc:
                if url_path == base_path or url_path.startswith(base_path + '/') or base_path == '/':
                    return True
        return len(self.base_urls) == 0

    def should_process_url(self, url: str) -> bool:
        """Check if URL should be processed based on config and scope"""
        # Scope check: must be under base URL path
        if not self._is_in_scope(url):
            return False

        parsed = urlparse(url)
        path = parsed.path.lower()

        # Check if URL has allowed extension
        for ext in self.config.file_extensions:
            if path.endswith(ext):
                return True

        # Also process HTML pages by default
        if not "." in path.split("/")[-1]:
            return True

        return False

    def extract_body_content(self, text: str) -> str:
        """Extract main content: keep '当前位置：' line and everything after, remove footer"""
        lines = text.split('\n')

        # Find '当前位置：' marker
        start_idx = -1
        for i, line in enumerate(lines):
            if '当前位置：' in line.strip() or '当前位置:' in line.strip():
                start_idx = i
                break

        # If found, keep from that line onwards
        if start_idx >= 0:
            lines = lines[start_idx:]
            # Merge breadcrumb lines into first line: '当前位置：首页 > 栏目'
            breadcrumb = lines[0].strip()
            merge_end = 1
            while merge_end < len(lines):
                stripped = lines[merge_end].strip()
                # compute logical text length ignoring URL parts of markdown links
                text_only = re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', stripped)
                if stripped == '' or text_only == '>' or text_only == '\>' or (len(text_only) < 30 and not any(c in text_only for c in '。，；')):
                    if stripped:
                        breadcrumb += ' ' + stripped
                    merge_end += 1
                else:
                    break
            lines = [breadcrumb] + lines[merge_end:]

        # Remove footer: find common footer markers
        footer_markers = ['关于我们', '版权所有', '京ICP备', '京公网安备', '区招生考试中心']
        end_idx = len(lines)
        for i, line in enumerate(lines):
            if i == 0:
                continue  # Skip '当前位置' line itself
            stripped = line.strip()
            for marker in footer_markers:
                if marker in stripped:
                    end_idx = i
                    break
            if end_idx != len(lines):
                break
        lines = lines[:end_idx]

        # Clean up: remove excessive blank lines
        result = '\n'.join(lines).strip()
        result = re.sub(r'\n{3,}', '\n\n', result)

        return result

    def _table_to_markdown(self, table) -> str:
        """Convert a BeautifulSoup <table> element directly to Markdown table format.
        This is more reliable than html2text for complex/styled tables.
        Uses a 2D grid to correctly handle rowspan and colspan alignment."""
        grid = {}
        max_col = -1
        max_row = -1
        
        trs = table.find_all('tr')
        for r_idx, tr in enumerate(trs):
            c_idx = 0
            for cell in tr.find_all(['td', 'th']):
                # Find next available column slot in this row
                while grid.get((r_idx, c_idx)) is not None:
                    c_idx += 1
                
                # Preserve line breaks for markdown cells
                for br in cell.find_all(['br', 'hr']):
                    br.replace_with(' <br> ')
                for p_div in cell.find_all(['p', 'div']):
                    p_div.insert_after(' <br> ')
                    p_div.unwrap()
                
                # Get pure text, collapse whitespace
                text = cell.get_text(strip=True)
                text = re.sub(r'\s+', ' ', text)
                
                # Clean up redundant <br> tags
                text = re.sub(r'(<br>\s*)+', '<br>', text)
                text = text.strip('<br>').strip()
                
                # Escape pipe characters inside cell text
                text = text.replace('|', '\\|')
                
                # Handle cell spanning
                try:
                    colspan = int(cell.get('colspan', 1))
                except (ValueError, TypeError):
                    colspan = 1
                try:
                    rowspan = int(cell.get('rowspan', 1))
                except (ValueError, TypeError):
                    rowspan = 1
                    
                # Fill grid with cell content and empty spans
                for r in range(r_idx, r_idx + rowspan):
                    for c in range(c_idx, c_idx + colspan):
                        grid[(r, c)] = text if (r == r_idx and c == c_idx) else ""
                        max_col = max(max_col, c)
                        max_row = max(max_row, r)
                
                c_idx += colspan

        if max_row < 0 or max_col < 0:
            return ""

        rows = []
        for r in range(max_row + 1):
            row = []
            for c in range(max_col + 1):
                row.append(grid.get((r, c), ""))
            rows.append(row)

        if not rows:
            return ''

        # Normalize column count (pad short rows)
        max_cols = max(len(r) for r in rows)
        for row in rows:
            while len(row) < max_cols:
                row.append('')

        # Build markdown table
        lines = []
        # First row as header
        lines.append('| ' + ' | '.join(rows[0]) + ' |')
        # Separator
        lines.append('| ' + ' | '.join(['---'] * max_cols) + ' |')
        # Data rows
        for row in rows[1:]:
            lines.append('| ' + ' | '.join(row) + ' |')

        return '\n'.join(lines)

    def _convert_tables_to_markdown(self, html: str) -> tuple:
        """Replace all <table> elements in HTML with their Markdown equivalents.
        This ensures tables are correctly formatted regardless of html2text behavior."""
        soup = BeautifulSoup(html, "html.parser")
        placeholders = {}
        for i, table in enumerate(soup.find_all('table')):
            md_table = self._table_to_markdown(table)
            if md_table:
                # Replace the table with a text placeholder to be swapped after html2text
                placeholder_key = f"___TABLE_PLACEHOLDER_{i}___"
                placeholders[placeholder_key] = md_table
                
                marker = soup.new_string(f"\n\n{placeholder_key}\n\n")
                table.replace_with(marker)
            else:
                table.decompose()
        return str(soup), placeholders

    def _clean_html_inline_tags(self, html: str) -> str:
        """Clean HTML at DOM level: unwrap meaningless inline tags.
        The HTML block structure (<p>, <div>, etc.) already defines the correct
        paragraph/line breaks - we just need to remove inline noise tags.
        """
        soup = BeautifulSoup(html, "html.parser")

        # Unwrap inline style tags that carry no semantic meaning for RAG
        for tag_name in ['span', 'font', 'u', 'b', 'strong', 'i', 'em']:
            for tag in soup.find_all(tag_name):
                tag.unwrap()

        # Removed the logic that flattens <p> inside <p> as it destroys line breaks.
        # html2text natively handles nested blocks very well.

        return str(soup)

    def html_to_markdown(self, html: str, base_url: str = "") -> str:
        """Convert HTML to Markdown format"""
        # Clean up complex table cells before conversion
        html, table_placeholders = self._convert_tables_to_markdown(html)
        # Remove meaningless inline tags that cause unwanted line breaks
        html = self._clean_html_inline_tags(html)
        
        result_md = ""
        if HTML2TEXT_AVAILABLE:
            h = html2text.HTML2Text()
            h.baseurl = base_url
            h.ignore_links = False
            h.ignore_images = True   # Removed images for RAG optimization
            h.body_width = 0  # Don't wrap lines
            h.bypass_tables = False
            h.pad_tables = True  # Pad table cells for alignment
            result_md = h.handle(html)
        else:
            # Fallback: simple HTML tag removal
            soup = BeautifulSoup(html, "html.parser")
            result_md = soup.get_text(separator="\n", strip=True)
            
        # Restore tables from placeholders
        for placeholder, md_table in table_placeholders.items():
            result_md = result_md.replace(placeholder, "\n\n" + md_table + "\n\n")
            
        return result_md

    def extract_title(self, soup: BeautifulSoup) -> str:
        """Extract title from HTML page"""
        # Try h1 first
        h1 = soup.find("h1")
        if h1:
            return h1.get_text(strip=True)

        # Try title tag
        title = soup.find("title")
        if title:
            return title.get_text(strip=True)

        return "Untitled"

    def extract_links(self, soup: BeautifulSoup, base_url: str) -> list:
        """Extract all links from HTML page, filtered by scope"""
        links = set()
        for a in soup.find_all("a", href=True):
            href = a["href"].strip()
            if not href or href.startswith(('javascript:', 'mailto:', 'tel:', '#')):
                continue
            full_url = urljoin(base_url, href)
            normalized = self._normalize_url(full_url)
            # Must be same domain AND within base URL scope
            if urlparse(normalized).netloc == urlparse(base_url).netloc and self._is_in_scope(normalized):
                links.add(normalized)
        return list(links)

    def process_page(self, url: str) -> bool:
        """Process a single page (called from BFS queue)"""
        normalized = self._normalize_url(url)

        # Thread-safe check and claim
        with self._lock:
            if self.page_count >= self.MAX_PAGES:
                return False
            if normalized in self.visited_urls:
                return False
            if not self.should_process_url(normalized):
                return False
            self.visited_urls.add(normalized)
            self.page_count += 1
            count = self.page_count
            depth = self.url_depths.get(normalized, 0)

        print(f"[{depth}] ({count}/{self.MAX_PAGES}) Crawling: {url}")

        MAX_RETRIES = 3
        response = None
        for attempt in range(MAX_RETRIES):
            try:
                response = self.session.get(url, timeout=30)
                # Fail fast on 404, don't retry
                if response.status_code == 404:
                    response.raise_for_status()
                    
                response.raise_for_status()
                break  # Success
            except requests.RequestException as e:
                # If it's a client error (e.g. 404), don't retry
                if hasattr(e, 'response') and e.response is not None and 400 <= e.response.status_code < 500 and e.response.status_code != 429:
                    with self._lock:
                        self.error_count += 1
                    print(f"  -> Error: {e}")
                    return False
                    
                if attempt < MAX_RETRIES - 1:
                    wait_time = 2 * (attempt + 1)
                    print(f"  -> Retry {attempt+1}/{MAX_RETRIES} for {url} (Wait {wait_time}s) due to error...")
                    time.sleep(wait_time)
                    continue
                else:
                    with self._lock:
                        self.error_count += 1
                    print(f"  -> Error after {MAX_RETRIES} tries: {e}")
                    return False
        # Fix encoding: use apparent_encoding if requests guessed wrong
        if response.encoding and response.encoding.lower() == 'iso-8859-1':
            response.encoding = response.apparent_encoding

        content_type = response.headers.get("Content-Type", "text/html").split(";")[0].strip()

        # Process based on content type
        if "text/html" in content_type:
            soup = BeautifulSoup(response.text, "html.parser")
            title = self.extract_title(soup)

            # Convert to markdown for RAG
            raw_content = self.html_to_markdown(response.text, url)
            # Extract main body content (remove nav/footer)
            content = self.extract_body_content(raw_content)
            # Prepend source URL to content
            content = f"> 来源: {url}\n\n{content}"

            # Generate filename from URL
            parsed = urlparse(url)
            filename = re.sub(r"[^\w\-]", "_", parsed.path.strip("/").replace("/", "_") or "index")

            # Save content (thread-safe via lock)
            with self._lock:
                status = self.storage.save_content(
                    filename=filename,
                    content=content,
                    source_url=url,
                    title=title,
                    content_type="text/markdown",
                    raw_html=response.text
                )

                if status == "new":
                    full_name = self.current_subdir + '/' + filename
                    self.new_files.append(full_name)
                    print(f"  -> New: {filename}")
                elif status == "updated":
                    full_name = self.current_subdir + '/' + filename
                    self.updated_files.append(full_name)
                    print(f"  -> Updated: {filename}")
                elif status == "unchanged":
                    self.unchanged_count += 1
                    print(f"  -> Unchanged: {filename}")

            # Collect sub-links for BFS queue
            if self.config.recursive and depth < self.config.max_depth:
                links = self.extract_links(soup, url)
                with self._lock:
                    for link in links:
                        link_norm = self._normalize_url(link)
                        if link_norm not in self.visited_urls and link_norm not in self.url_depths:
                            self.url_depths[link_norm] = depth + 1
                            self.bfs_queue.append(link)

        return True

        # Exception handled inside retry loop

    def _get_delay(self) -> float:
        """Get current delay, checking for real-time updates from .crawl_delay file"""
        try:
            delay_file = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), '.crawl_delay')
            if os.path.exists(delay_file):
                with open(delay_file, 'r') as f:
                    val = float(f.read().strip())
                    if val != self.config.delay:
                        print(f"  [delay updated: {self.config.delay}s -> {val}s]")
                        self.config.delay = val
                    return val
        except (ValueError, IOError):
            pass
        return self.config.delay

    def _crawl_worker(self, url: str):
        """Worker function for concurrent crawling"""
        self.process_page(url)
        time.sleep(self._get_delay())

    def _get_url_subdir(self, url: str) -> str:
        """Extract domain as subdirectory name from URL"""
        parsed = urlparse(url)
        return parsed.netloc or 'default'

    def crawl(self) -> CrawlResult:
        """Start crawling from configured URLs using BFS with concurrent workers"""
        print(f"Starting crawl with {len(self.config.urls)} URLs ({self.CONCURRENT_WORKERS} workers)")
        print(f"Output directory: {self.config.output_dir}")
        self.base_urls = list(self.config.urls)  # set scope to configured URLs

        try:
            for url in self.config.urls:
                # Create subdirectory per target URL domain
                subdir = self._get_url_subdir(url)
                self.current_subdir = subdir
                sub_output = os.path.join(self.config.output_dir, subdir)
                self.storage = StorageManager(sub_output, self.config.enable_meta)
                
                print(f"\nCrawling from: {url} -> {subdir}/")
                
                # Initialize BFS with seed URL
                self.bfs_queue = deque([url])
                self.url_depths = {self._normalize_url(url): 0}
                
                # BFS with thread pool
                with ThreadPoolExecutor(max_workers=self.CONCURRENT_WORKERS) as executor:
                    active_futures = set()
                    
                    while True:
                        # Submit new tasks from the queue
                        with self._lock:
                            while self.bfs_queue and len(active_futures) < self.CONCURRENT_WORKERS and self.page_count < self.MAX_PAGES:
                                next_url = self.bfs_queue.popleft()
                                future = executor.submit(self._crawl_worker, next_url)
                                active_futures.add(future)
                        
                        if not active_futures:
                            # No active work and queue is empty - done
                            with self._lock:
                                if not self.bfs_queue:
                                    break
                                else:
                                    continue
                        
                        # Wait for at least one to complete
                        done = set()
                        for f in as_completed(active_futures):
                            done.add(f)
                            break  # Process one at a time to refill queue quickly
                        
                        active_futures -= done
                        
                        # Check if we hit the limit
                        with self._lock:
                            if self.page_count >= self.MAX_PAGES:
                                # Cancel remaining and drain
                                for f in active_futures:
                                    f.cancel()
                                break
                
                # Finalize this URL's storage
                self.storage.finalize(self.new_files, self.updated_files, [])

            return CrawlResult(
                success=True,
                new_files=self.new_files,
                updated_files=self.updated_files,
                deleted_files=[],
                message=f"Crawl completed. Total: {self.page_count}, New: {len(self.new_files)}, Updated: {len(self.updated_files)}, Unchanged: {self.unchanged_count}, Errors: {self.error_count}"
            )

        except Exception as e:
            return CrawlResult(
                success=False,
                new_files=self.new_files,
                updated_files=self.updated_files,
                deleted_files=[],
                message=f"Crawl failed: {str(e)}"
            )


def main():
    """CLI entry point"""
    if len(sys.argv) < 2:
        print("Usage: python crawler.py <config_path>")
        sys.exit(1)

    config_path = sys.argv[1]
    config = CrawlerConfig.from_yaml(config_path)

    crawler = WebCrawler(config)
    result = crawler.crawl()

    # Output JSON result for Tauri
    print("\n=== RESULT ===")
    print(json.dumps(asdict(result), ensure_ascii=False))


if __name__ == "__main__":
    main()
