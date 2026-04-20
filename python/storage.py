"""
Storage manager for DPCrawler - handles file metadata and indexing
"""
import os
import json
import hashlib
from datetime import datetime
from dataclasses import dataclass, asdict
from typing import Dict, List, Optional


@dataclass
class FileMeta:
    """File metadata"""
    md5: str
    fetch_date: str
    source_url: str
    title: str
    file_size: int
    content_type: str

    @classmethod
    def from_dict(cls, data: dict) -> "FileMeta":
        return cls(
            md5=data.get("md5", ""),
            fetch_date=data.get("fetch_date", ""),
            source_url=data.get("source_url", ""),
            title=data.get("title", ""),
            file_size=data.get("file_size", 0),
            content_type=data.get("content_type", ""),
        )


@dataclass
class CrawlResult:
    """Crawl result summary"""
    success: bool
    new_files: List[str]
    updated_files: List[str]
    deleted_files: List[str]
    message: str


class StorageManager:
    """Manages file storage, metadata, and indexing"""

    def __init__(self, output_dir: str, enable_meta: bool = True):
        self.output_dir = output_dir
        self.enable_meta = enable_meta
        self.index: Dict[str, FileMeta] = {}
        self._load_index()

    def _load_index(self):
        """Load existing index from disk"""
        index_path = os.path.join(self.output_dir, "index.json")
        if os.path.exists(index_path):
            try:
                with open(index_path, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    for name, meta in data.get("file_tree", {}).items():
                        self.index[name] = FileMeta.from_dict(meta)
            except (json.JSONDecodeError, IOError):
                pass

    def _save_index(self, new_files: List[str], updated_files: List[str], deleted_files: List[str]):
        """Save updated index to disk"""
        index_path = os.path.join(self.output_dir, "index.json")
        file_tree = {name: asdict(meta) for name, meta in self.index.items()}

        data = {
            "last_updated": datetime.now().isoformat(),
            "total_files": len(self.index),
            "new_files": new_files,
            "updated_files": updated_files,
            "deleted_files": deleted_files,
            "file_tree": file_tree,
        }

        os.makedirs(self.output_dir, exist_ok=True)
        with open(index_path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    @staticmethod
    def compute_md5(content: str) -> str:
        """Compute MD5 hash of content"""
        return hashlib.md5(content.encode("utf-8")).hexdigest()

    def save_content(
        self, 
        filename: str, 
        content: str, 
        source_url: str, 
        title: str = "", 
        content_type: str = "text/html", 
        raw_html: str = "",
        raw_bytes: bytes = None,
        original_ext: str = "",
        html_content: str = ""
    ) -> Optional[str]:
        """Save content and its metadata, returns file status: 'new', 'updated', 'unchanged', or None on error. Optionally dumps html_content to html_views."""
        try:
            docs_dir = os.path.join(self.output_dir, "docs")
            meta_dir = os.path.join(self.output_dir, "meta")
            os.makedirs(docs_dir, exist_ok=True)

            # Compute file path and extension
            ext = self._get_extension(content_type, filename)
            base_filename = self._sanitize_filename(filename)
            content_path = os.path.join(docs_dir, f"{base_filename}{ext}")

            # Compute MD5 from raw bytes or HTML for accurate change detection
            if raw_bytes is not None:
                new_md5 = hashlib.md5(raw_bytes).hexdigest()
            else:
                md5_source = raw_html if raw_html else content
                new_md5 = self.compute_md5(md5_source)

            # Check if file is unchanged
            if base_filename in self.index and self.index[base_filename].md5 == new_md5:
                return "unchanged"

            # Save content file
            with open(content_path, "w", encoding="utf-8") as f:
                f.write(content)

            file_size = os.path.getsize(content_path)

            # Save metadata file
            if self.enable_meta:
                os.makedirs(meta_dir, exist_ok=True)
                meta = FileMeta(
                    md5=new_md5,
                    fetch_date=datetime.now().isoformat(),
                    source_url=source_url,
                    title=title or base_filename,
                    file_size=file_size,
                    content_type=content_type,
                )
                meta_path = os.path.join(meta_dir, f"{base_filename}.json")
                with open(meta_path, "w", encoding="utf-8") as f:
                    json.dump(asdict(meta), f, ensure_ascii=False, indent=2)

            # Save raw document if available
            raw_dir = os.path.join(self.output_dir, "raw")
            if raw_bytes is not None:
                os.makedirs(raw_dir, exist_ok=True)
                raw_file_ext = original_ext if original_ext else self._get_extension(content_type, filename)
                raw_path = os.path.join(raw_dir, f"{base_filename}{raw_file_ext}")
                with open(raw_path, "wb") as bf:
                    bf.write(raw_bytes)
            elif raw_html:
                os.makedirs(raw_dir, exist_ok=True)
                raw_path = os.path.join(raw_dir, f"{base_filename}.html")
                with open(raw_path, "w", encoding="utf-8") as tf:
                    tf.write(raw_html)

            # Save clean high-fidelity HTML for frontend UI previews
            if html_content:
                html_views_dir = os.path.join(self.output_dir, "html_views")
                os.makedirs(html_views_dir, exist_ok=True)
                html_path = os.path.join(html_views_dir, f"{base_filename}.html")
                with open(html_path, "w", encoding="utf-8") as hf:
                    hf.write(html_content)

            # Update index
            was_new = base_filename not in self.index
            self.index[base_filename] = FileMeta(
                md5=new_md5,
                fetch_date=datetime.now().isoformat(),
                source_url=source_url,
                title=title or base_filename,
                file_size=file_size,
                content_type=content_type,
            )

            return "new" if was_new else "updated"

        except Exception as e:
            print(f"Error saving {filename}: {e}")
            return None

    def finalize(self, new_files: List[str], updated_files: List[str], deleted_files: List[str]):
        """Finalize crawl session, save index"""
        self._save_index(new_files, updated_files, deleted_files)

    @staticmethod
    def _sanitize_filename(filename: str) -> str:
        """Sanitize filename for filesystem"""
        # Remove invalid characters
        import re
        filename = re.sub(r'[<>:"/\\|?*]', "_", filename)
        # Limit length
        if len(filename) > 200:
            filename = filename[:200]
        return filename or "untitled"

    @staticmethod
    def _get_extension(content_type: str, filename: str) -> str:
        """Determine file extension from content type or filename"""
        content_type_map = {
            # Web
            "text/html": ".html",
            "text/markdown": ".md",
            "text/plain": ".txt",
            "text/csv": ".csv",
            "text/xml": ".xml",
            "application/json": ".json",
            "application/xml": ".xml",
            # Documents
            "application/pdf": ".pdf",
            "application/msword": ".doc",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document": ".docx",
            "application/vnd.ms-excel": ".xls",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": ".xlsx",
            "application/vnd.ms-powerpoint": ".ppt",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation": ".pptx",
            "application/rtf": ".rtf",
            "application/epub+zip": ".epub",
        }
        if content_type in content_type_map:
            return content_type_map[content_type]

        # Try to extract from filename
        known_exts = {
            ".html", ".htm", ".md", ".txt", ".csv", ".xml", ".json",
            ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
            ".rtf", ".odt", ".ods", ".odp", ".epub", ".mobi",
            ".tex", ".latex", ".rst", ".log", ".yaml", ".yml",
        }
        if "." in filename:
            ext = "." + filename.rsplit(".", 1)[-1]
            if ext.lower() in known_exts:
                return ext.lower()
        return ".html"
