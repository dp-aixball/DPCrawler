"""
Configuration loader for DPCrawler
"""
import os
from dataclasses import dataclass, field
from typing import List
import yaml


@dataclass
class CrawlerConfig:
    """Crawler configuration"""
    urls: List[str] = field(default_factory=list)
    file_extensions: List[str] = field(default_factory=lambda: [".html", ".pdf", ".doc", ".md"])
    content_format: str = "markdown"
    meta_format: str = "json"
    enable_meta: bool = True
    index_file: str = "index.json"
    output_dir: str = "./output"
    delay: float = 1.0
    max_workers: int = 3
    recursive: bool = True
    max_depth: int = 3
    min_year: int = 0  # 0 means no filtering

    @classmethod
    def from_yaml(cls, path: str) -> "CrawlerConfig":
        """Load configuration from YAML file"""
        if not os.path.exists(path):
            return cls()

        with open(path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f) or {}

        crawler_data = data.get("crawler", {})
        return cls(
            urls=crawler_data.get("urls", []),
            file_extensions=crawler_data.get("file_extensions", [".html", ".pdf", ".doc", ".md"]),
            content_format=crawler_data.get("content_format", "markdown"),
            meta_format=crawler_data.get("meta_format", "json"),
            enable_meta=crawler_data.get("enable_meta", True),
            index_file=crawler_data.get("index_file", "index.json"),
            output_dir=crawler_data.get("output_dir", "./output"),
            delay=crawler_data.get("delay", 1.0),
            max_workers=crawler_data.get("max_workers", 3),
            recursive=crawler_data.get("recursive", True),
            max_depth=crawler_data.get("max_depth", 3),
            min_year=crawler_data.get("min_year", 0),
        )

    def to_yaml(self, path: str):
        """Save configuration to YAML file"""
        data = {
            "crawler": {
                "urls": self.urls,
                "file_extensions": self.file_extensions,
                "content_format": self.content_format,
                "meta_format": self.meta_format,
                "enable_meta": self.enable_meta,
                "index_file": self.index_file,
                "output_dir": self.output_dir,
                "delay": self.delay,
                "max_workers": self.max_workers,
                "recursive": self.recursive,
                "max_depth": self.max_depth,
                "min_year": self.min_year,
            }
        }
        os.makedirs(os.path.dirname(path) if os.path.dirname(path) else ".", exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            yaml.dump(data, f, allow_unicode=True, default_flow_style=False)
