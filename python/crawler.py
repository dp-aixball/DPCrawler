"""
Main crawler module for DPCrawler
"""
import os
import sys
import json
import time
import ssl
import threading
from collections import deque
from concurrent.futures import ThreadPoolExecutor, as_completed
from urllib.parse import urljoin, urlparse
from typing import Set, Optional, Dict
import re

import requests
from bs4 import BeautifulSoup
import parsers

from config import CrawlerConfig
from storage import StorageManager, CrawlResult
from dataclasses import asdict


class WebCrawler:
    """Web crawler for RAG knowledge collection"""

    MAX_PAGES = 10000  # Maximum pages per crawl session

    CONCURRENT_WORKERS = 8  # Parallel crawl workers

    # Constants for parsing
    BINARY_CONVERTERS = {
        "application/pdf": ("pdf_to_markdown", ".pdf"),
        "application/msword": ("docx_to_markdown", ".doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document": ("docx_to_markdown", ".docx"),
        "application/vnd.ms-excel": ("xlsx_to_markdown", ".xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": ("xlsx_to_markdown", ".xlsx"),
        "application/vnd.ms-powerpoint": ("pptx_to_markdown", ".ppt"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation": ("pptx_to_markdown", ".pptx"),
    }
    EXT_TO_CONVERTER = {
        ".pdf": "pdf_to_markdown",
        ".doc": "docx_to_markdown",
        ".docx": "docx_to_markdown",
        ".xls": "xlsx_to_markdown",
        ".xlsx": "xlsx_to_markdown",
        ".ppt": "pptx_to_markdown",
        ".pptx": "pptx_to_markdown",
    }
    BINARY_EXTENSIONS = set(EXT_TO_CONVERTER.keys())
    PLAINTEXT_TYPES = {"text/plain", "text/csv", "application/json", "application/xml", "text/xml", "application/yaml", "application/x-yaml"}
    PLAINTEXT_EXTENSIONS = {".txt", ".csv", ".json", ".xml", ".yaml", ".yml", ".md"}

    def __init__(self, config: CrawlerConfig):
        self.config = config
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": "Mozilla/5.0 (compatible; DPCrawler/1.0; +https://github.com/dpcrawler)"
        })
        # Configure SSL adapter for compatibility with problematic certificates
        import urllib3
        from requests.adapters import HTTPAdapter
        from urllib3.util.ssl_ import create_urllib3_context
        
        class SSLAdapter(HTTPAdapter):
            def init_poolmanager(self, *args, **kwargs):
                # Force TLS 1.2 for compatibility with government sites
                context = ssl.SSLContext(ssl.PROTOCOL_TLSv1_2)
                context.set_ciphers('DEFAULT:@SECLEVEL=0')
                context.check_hostname = False
                context.verify_mode = ssl.CERT_NONE
                kwargs['ssl_context'] = context
                return super().init_poolmanager(*args, **kwargs)
        
        self.session.mount('https://', SSLAdapter())
        self.session.verify = False
        urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)
        self.ssl_fallback_hosts: Set[str] = set()  # Hosts that need HTTP fallback
        self.host_semaphores: Dict[str, threading.Semaphore] = {}  # Per-host concurrency limit
        self._host_lock = threading.Lock()
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
        self._crawl_stopped = False  # Stop flag for main crawl

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

        # Always allow .html/.htm pages regardless of extension config
        if path.endswith(('.html', '.htm')):
            return True

        return False

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

    @staticmethod
    def extract_max_year(text: str) -> int:
        """Extract max year from text matching strict date patterns. Returns 0 if no match.
        Formats: YYYY年, YYYY-MM, YYYY/MMDD, YYYY_MMDD
        Year: 1990-2099, Month: 01-12 (2-digit), Day: 01-31 (2-digit).
        """
        MONTH = r'(?:0[1-9]|1[0-2])'       # 01-12
        DAY = r'(?:0[1-9]|[12]\d|3[01])'    # 01-31
        YEAR = r'((?:19|20)\d{2})'           # 1990-2099

        years = []
        # YYYY年 (e.g. 2025年)
        for y in re.findall(YEAR + r'年', text):
            years.append(int(y))
        # YYYY-MM (e.g. 2025-03)
        for y in re.findall(YEAR + r'-' + MONTH + r'(?:\b|[^0-9])', text):
            years.append(int(y))
        # YYYY/MMDD (e.g. 2025/0118)
        for y in re.findall(YEAR + r'/' + MONTH + DAY + r'(?:\b|[^0-9])', text):
            years.append(int(y))
        # YYYY/MM (e.g. 2025/03)
        for y in re.findall(YEAR + r'/' + MONTH + r'(?:\b|[^0-9])', text):
            years.append(int(y))
        # YYYY_MMDD (e.g. 2025_1120)
        for y in re.findall(YEAR + r'_' + MONTH + DAY + r'(?:\b|[^0-9])', text):
            years.append(int(y))
        # YYYY_MM (e.g. 2025_03)
        for y in re.findall(YEAR + r'_' + MONTH + r'(?:\b|[^0-9])', text):
            years.append(int(y))
        if not years:
            return 0
        return max(years)

    def process_page(self, url: str) -> bool:
        """Process a single page (called from BFS queue)"""
        normalized = self._normalize_url(url)

        # URL year filter: check before claiming the page
        min_year = self.config.min_year
        if min_year:
            min_year = int(min_year)
            url_max_year = self.extract_max_year(url)
            if url_max_year > 0 and url_max_year < min_year:
                with self._lock:
                    print(f"  -> [skip] URL year {url_max_year} < {min_year}: {url}")
                return False

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

        # Auto-downgrade to HTTP for known SSL-problematic hosts
        parsed = urlparse(url)
        if parsed.hostname in self.ssl_fallback_hosts and url.startswith('https://'):
            url = 'http://' + url[len('https://'):]

        # Per-host concurrency limit: max 2 concurrent requests per host
        host_sem = None
        if parsed.hostname:
            with self._host_lock:
                if parsed.hostname not in self.host_semaphores:
                    self.host_semaphores[parsed.hostname] = threading.Semaphore(2)
                host_sem = self.host_semaphores[parsed.hostname]

        acquired = True  # Track if semaphore was acquired
        if host_sem:
            acquired = host_sem.acquire(timeout=30)
            if not acquired:
                print(f"  -> [timeout] Host semaphore wait exceeded: {url}")
                return False

        response = None
        try:
            MAX_RETRIES = 3
            for attempt in range(MAX_RETRIES):
                try:
                    response = self.session.get(url, timeout=60)
                    if response.status_code == 404:
                        response.raise_for_status()
                    response.raise_for_status()
                    break
                except requests.RequestException as e:
                    err_str = str(e)
                    if 'BAD_ECPOINT' in err_str or ('SSL' in err_str.upper() and 'SSLError' in err_str):
                        if url.startswith('https://'):
                            http_url = 'http://' + url[len('https://'):]
                            print(f"  -> SSL error, trying HTTP fallback: {http_url}")
                            try:
                                response = self.session.get(http_url, timeout=60)
                                if response.status_code < 400 or len(response.text) > 500:
                                    host = urlparse(url).hostname
                                    if host:
                                        self.ssl_fallback_hosts.add(host)
                                        print(f"  -> Registered {host} for HTTP fallback")
                                    url = http_url
                                    break
                                response.raise_for_status()
                            except requests.RequestException as http_e:
                                try:
                                    headers = {'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'}
                                    response = self.session.get(http_url, timeout=60, headers=headers)
                                    if len(response.text) > 500:
                                        url = http_url
                                        break
                                except:
                                    pass
                                with self._lock:
                                    self.error_count += 1
                                print(f"  -> SSL Error and HTTP fallback both failed: {e}")
                                return False
                        else:
                            with self._lock:
                                self.error_count += 1
                            print(f"  -> SSL Error (unrecoverable): {e}")
                            return False
                    if hasattr(e, 'response') and e.response is not None and 400 <= e.response.status_code < 500 and e.response.status_code != 429:
                        with self._lock:
                            self.error_count += 1
                        print(f"  -> Error: {e}")
                        return False
                    if attempt < MAX_RETRIES - 1:
                        wait_time = 2 * (attempt + 1)
                        print(f"  -> Retry {attempt+1}/{MAX_RETRIES} for {url} (Wait {wait_time}s)...")
                        time.sleep(wait_time)
                    else:
                        with self._lock:
                            self.error_count += 1
                        print(f"  -> Error after {MAX_RETRIES} tries: {e}")
                        return False
        finally:
            if host_sem and acquired:
                host_sem.release()

        if response.encoding and response.encoding.lower() == 'iso-8859-1':
            response.encoding = response.apparent_encoding

        content_type = response.headers.get("Content-Type", "text/html").split(";")[0].strip()
        url_ext = os.path.splitext(urlparse(url).path)[1].lower()
        parsed = urlparse(url)
        filename = re.sub(r"[^\w\-]", "_", parsed.path.strip("/").replace("/", "_") or "index")

        content = None
        title = "Untitled"
        raw_html = None
        sub_links = []

        if "text/html" in content_type and url_ext not in self.BINARY_EXTENSIONS:
            soup = BeautifulSoup(response.text, "html.parser")
            title = parsers.extract_title(soup)
            raw_html = response.text
            raw_content = parsers.html_to_markdown(response.text, url)
            content = parsers.extract_body_content(raw_content)
            if self.config.recursive and depth < self.config.max_depth:
                sub_links = self.extract_links(soup, url)

        elif content_type in self.BINARY_CONVERTERS or url_ext in self.BINARY_EXTENSIONS:
            converter_name = None
            if content_type in self.BINARY_CONVERTERS:
                converter_name, _ = self.BINARY_CONVERTERS[content_type]
            elif url_ext in self.EXT_TO_CONVERTER:
                converter_name = self.EXT_TO_CONVERTER[url_ext]
            if converter_name:
                try:
                    converter = getattr(parsers, converter_name)
                    with ThreadPoolExecutor(max_workers=1) as conv_exec:
                        content = conv_exec.submit(converter, response.content).result(timeout=60)
                    title = os.path.basename(parsed.path) or filename
                except Exception as e:
                    if isinstance(e, TimeoutError):
                        print(f"  -> Conversion timeout (60s) for {url_ext}: {url}")
                    else:
                        print(f"  -> Conversion error ({url_ext}): {e}")
                    return False
            else:
                print(f"  -> Skipped (unsupported binary: {content_type})")
                return False

        elif content_type in self.PLAINTEXT_TYPES or url_ext in self.PLAINTEXT_EXTENSIONS:
            text = response.text.strip()
            title = os.path.basename(parsed.path) or filename
            if url_ext in {'.json', '.xml', '.yaml', '.yml', '.csv'}:
                lang = url_ext.lstrip('.')
                if lang == 'yml':
                    lang = 'yaml'
                content = f"```{lang}\n{text}\n```"
            else:
                content = text

        else:
            print(f"  -> Skipped (unsupported: {content_type})")
            return False

        if content is None:
            return False

        # Content year filter: if content has year patterns and max < threshold, skip saving but still crawl sub-links
        content_skip = False
        if min_year:
            content_max_year = self.extract_max_year(content)
            if content_max_year > 0 and content_max_year < min_year:
                print(f"  -> [skip] Content year {content_max_year} < {min_year}: {url}")
                content_skip = True

        if not content_skip:
            content = f"> 来源: {url}\n\n{content}"

            with self._lock:
                status = self.storage.save_content(
                    filename=filename,
                    content=content,
                    source_url=url,
                    title=title,
                    content_type="text/markdown",
                    raw_html=raw_html
                )

                if status == "new":
                    full_name = self.current_subdir + '/' + filename
                    self.new_files.append(full_name)
                    print(f"  -> New: {full_name}")
                elif status == "updated":
                    full_name = self.current_subdir + '/' + filename
                    self.updated_files.append(full_name)
                    print(f"  -> Updated: {full_name}")
                elif status == "unchanged":
                    self.unchanged_count += 1
                    full_name = self.current_subdir + '/' + filename
                    print(f"  -> Unchanged: {full_name}")

        if sub_links:
            with self._lock:
                for link in sub_links:
                    link_norm = self._normalize_url(link)
                    if link_norm not in self.visited_urls and link_norm not in self.url_depths:
                        self.url_depths[link_norm] = depth + 1
                        self.bfs_queue.append(link)

        return True

    def _get_delay(self) -> float:
        """Get current delay, checking for real-time updates from .crawl_delay file"""
        try:
            delay_file = os.path.join(os.getcwd(), '.crawl_delay')
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

    def pre_crawl(self) -> dict:
        """Pre-crawl: BFS discovery of all URLs without downloading/storing content.
        Returns depth statistics for progress estimation."""
        print(f"Pre-crawl: discovering URLs from {len(self.config.urls)} seed(s) ({self.CONCURRENT_WORKERS} workers)")
        self.base_urls = list(self.config.urls)

        discovered = {}  # url -> depth
        visited = set()
        lock = threading.Lock()
        self._pre_crawl_stopped = False  # graceful stop flag
        host_semaphores = {}  # per-host concurrency limit
        host_lock = threading.Lock()

        def _handle_stop(signum, frame):
            print("\n[pre-crawl] Stop requested, saving current stats...")
            self._pre_crawl_stopped = True

        import signal
        signal.signal(signal.SIGTERM, _handle_stop)

        def sniff_worker(url: str, depth: int):
            """Fetch HTML page, extract links. Skip binary files."""
            norm = self._normalize_url(url)
            with lock:
                if norm in visited:
                    return []
                visited.add(norm)

            # Check if URL passes scope + extension filter
            if not self.should_process_url(norm):
                return []

            # For binary file URLs, just count them - don't fetch
            url_ext = os.path.splitext(urlparse(url).path)[1].lower()
            if url_ext in self.BINARY_EXTENSIONS:
                with lock:
                    discovered[norm] = depth
                count = len(discovered)
                print(f"  [pre-crawl] depth={depth} found={count} (doc) {url}")
                return []

            # Per-host concurrency limit: max 2 concurrent requests per host
            host = urlparse(url).hostname
            sem = None
            if host:
                with host_lock:
                    if host not in host_semaphores:
                        host_semaphores[host] = threading.Semaphore(2)
                    sem = host_semaphores[host]

            def _fetch_and_extract():
                # Fetch HTML pages to extract links
                response = None
                try:
                    response = self.session.get(url, timeout=15)
                    response.raise_for_status()
                except requests.RequestException as e:
                    err_str = str(e)
                    if 'BAD_ECPOINT' in err_str or ('SSL' in err_str.upper() and 'SSLError' in err_str):
                        if url.startswith('https://'):
                            http_url = 'http://' + url[len('https://'):]
                            try:
                                response = self.session.get(http_url, timeout=15)
                                if response.status_code < 400 or len(response.text) > 500:
                                    host2 = urlparse(url).hostname
                                    if host2:
                                        self.ssl_fallback_hosts.add(host2)
                                    url = http_url
                                else:
                                    response.raise_for_status()
                            except Exception:
                                return []
                    else:
                        return []

                if response is None:
                    return []

                content_type = response.headers.get("Content-Type", "").split(";")[0].strip()
                if "text/html" not in content_type:
                    with lock:
                        discovered[norm] = depth
                    count = len(discovered)
                    print(f"  [pre-crawl] depth={depth} found={count} {url}")
                    return []

                with lock:
                    discovered[norm] = depth
                count = len(discovered)
                print(f"  [pre-crawl] depth={depth} found={count} {url}")

                if response.encoding and response.encoding.lower() == 'iso-8859-1':
                    response.encoding = response.apparent_encoding
                soup = BeautifulSoup(response.text, "html.parser")
                child_links = []
                for link in self.extract_links(soup, url):
                    link_norm = self._normalize_url(link)
                    with lock:
                        if link_norm not in visited and link_norm not in discovered:
                            child_links.append((link, depth + 1))
                return child_links

            if sem:
                with sem:
                    return _fetch_and_extract()
            else:
                return _fetch_and_extract()

        def _build_result() -> dict:
            """Build statistics from current discovered state."""
            max_depth = max(discovered.values()) if discovered else 0
            urls_per_depth = {}
            for d in range(max_depth + 1):
                urls_per_depth[str(d)] = sum(1 for v in discovered.values() if v == d)
            by_depth = {}
            cumulative = 0
            for d in range(max_depth + 1):
                cumulative += urls_per_depth.get(str(d), 0)
                by_depth[str(d)] = cumulative
            return {
                "total": len(discovered),
                "by_depth": by_depth,
                "urls_per_depth": urls_per_depth,
                "max_depth": max_depth,
                "urls": list(discovered.keys()),
            }

        try:
            for seed_url in self.config.urls:
                if self._pre_crawl_stopped:
                    break
                print(f"\nPre-crawling from: {seed_url}")
                queue = deque([(seed_url, 0)])

                with ThreadPoolExecutor(max_workers=self.CONCURRENT_WORKERS) as executor:
                    active = set()

                    while True:
                        if self._pre_crawl_stopped:
                            executor.shutdown(wait=False, cancel_futures=True)
                            break
                        # Submit from queue
                        while queue and len(active) < self.CONCURRENT_WORKERS:
                            url, depth = queue.popleft()
                            future = executor.submit(sniff_worker, url, depth)
                            future._depth = depth
                            active.add(future)

                        if not active:
                            if not queue:
                                break
                            continue

                        # Wait for at least one to complete (with timeout to allow periodic checks)
                        done = set()
                        try:
                            for f in as_completed(active, timeout=2):
                                done.add(f)
                        except TimeoutError:
                            pass  # No futures completed in 2s, check stop flag and retry
                        active -= done

                        # Periodically check if stopped
                        if self._pre_crawl_stopped:
                            executor.shutdown(wait=False, cancel_futures=True)
                            break

                        for f in done:
                            try:
                                child_links = f.result()
                                for link, d in child_links:
                                    queue.append((link, d))
                            except Exception:
                                pass

        except Exception as e:
            print(f"Pre-crawl error: {e}")

        result = _build_result()
        was_stopped = self._pre_crawl_stopped

        if was_stopped:
            print(f"\nPre-crawl stopped early: {len(discovered)} URLs, max depth {result['max_depth']}")
        else:
            print(f"\nPre-crawl complete: {len(discovered)} URLs, max depth {result['max_depth']}")
        for d in range(result['max_depth'] + 1):
            print(f"  depth {d}: {result['urls_per_depth'].get(str(d), 0)} URLs (cumulative: {result['by_depth'].get(str(d), 0)})")

        return result

    def crawl(self) -> CrawlResult:
        """Start crawling from configured URLs using BFS with concurrent workers"""
        self._crawl_stopped = False

        def _handle_stop(signum, frame):
            print("\n[crawl] Stop requested...")
            self._crawl_stopped = True
            # Close session to abort in-flight requests immediately
            try:
                self.session.close()
            except:
                pass

        import signal
        signal.signal(signal.SIGTERM, _handle_stop)

        print(f"Starting crawl with {len(self.config.urls)} URLs ({self.CONCURRENT_WORKERS} workers)")
        print(f"Output directory: {self.config.output_dir}")
        # Normalize URLs: auto-add protocol prefix
        normalized_urls = []
        for u in self.config.urls:
            if not u.startswith('http://') and not u.startswith('https://'):
                u = 'https://' + u
            normalized_urls.append(u)
        self.config.urls = normalized_urls
        self.base_urls = list(normalized_urls)  # set scope to configured URLs

        try:
            for url in self.config.urls:
                # Create subdirectory per target URL domain
                subdir = self._get_url_subdir(url)
                self.current_subdir = subdir
                sub_output = os.path.join(self.config.output_dir, subdir)
                self.storage = StorageManager(sub_output, self.config.enable_meta)
                
                # Save crawl config to site subdirectory for later restoration
                crawl_config = {
                    "url": url,
                    "file_extensions": self.config.file_extensions,
                    "content_format": self.config.content_format,
                    "output_dir": self.config.output_dir,
                    "delay": self.config.delay,
                    "max_depth": self.config.max_depth,
                    "recursive": self.config.recursive,
                }
                os.makedirs(sub_output, exist_ok=True)
                with open(os.path.join(sub_output, "crawl_config.json"), "w", encoding="utf-8") as cf:
                    json.dump(crawl_config, cf, ensure_ascii=False, indent=2)
                
                print(f"\nCrawling from: {url} -> {subdir}/")
                
                # Initialize BFS with seed URL
                self.bfs_queue = deque([url])
                self.url_depths = {self._normalize_url(url): 0}
                
                # BFS with thread pool
                with ThreadPoolExecutor(max_workers=self.CONCURRENT_WORKERS) as executor:
                    active_futures = set()
                    
                    while True:
                        if self._crawl_stopped:
                            executor.shutdown(wait=False, cancel_futures=True)
                            break

                        # Submit new tasks from the queue
                        with self._lock:
                            while self.bfs_queue and len(active_futures) < self.CONCURRENT_WORKERS and self.page_count < self.MAX_PAGES:
                                next_url = self.bfs_queue.popleft()
                                future = executor.submit(self._crawl_worker, next_url)
                                active_futures.add(future)
                        
                        if not active_futures:
                            with self._lock:
                                if not self.bfs_queue:
                                    break
                                else:
                                    continue
                        
                        # Wait for at least one to complete (with timeout for stop check)
                        done = set()
                        try:
                            for f in as_completed(active_futures, timeout=2):
                                done.add(f)
                        except TimeoutError:
                            pass  # Check stop flag and retry
                        
                        if self._crawl_stopped:
                            executor.shutdown(wait=False, cancel_futures=True)
                            break
                        
                        active_futures -= done
                        
                        with self._lock:
                            if self.page_count >= self.MAX_PAGES:
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
        print("Usage: python crawler.py <config_path> [--pre-crawl]")
        sys.exit(1)

    config_path = sys.argv[1]
    pre_crawl_mode = '--pre-crawl' in sys.argv

    config = CrawlerConfig.from_yaml(config_path)
    crawler = WebCrawler(config)

    if pre_crawl_mode:
        result = crawler.pre_crawl()
        print("\n=== RESULT ===")
        print(json.dumps(result, ensure_ascii=False))
    else:
        result = crawler.crawl()
        print("\n=== RESULT ===")
        print(json.dumps(asdict(result), ensure_ascii=False))


if __name__ == "__main__":
    main()
