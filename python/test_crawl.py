import sys
import logging
from crawler import WebCrawler

logging.basicConfig(level=logging.DEBUG)

config = {
    'urls': ['https://www.bjeea.cn/'],
    'file_extensions': ['.pdf', '.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx', '.csv'],
    'content_format': 'markdown',
    'meta_format': 'json',
    'enable_meta': True,
    'index_file': 'index.json',
    'output_dir': './test_output',
    'delay': 0,
    'max_workers': 1,
    'recursive': True,
    'max_depth': 1,
    'min_year': 2024
}

crawler = WebCrawler(config)
crawler.start()
print("Done")
