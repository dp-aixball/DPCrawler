import fitz
import re
from collections import Counter

def _clean_text(t: str) -> str:
    return t.replace('\n', '').strip()

def extract_gov_pdf_to_markdown(pdf_path: str) -> str:
    """
    A heuristic layout-preserving PDF to Markdown parser specifically tuned 
    for Chinese government documents.
    """
    doc = fitz.open(pdf_path)
    
    all_sizes = []
    blocks_data = []

    for page_idx in range(len(doc)):
        page = doc[page_idx]
        page_dict = page.get_text("dict")
        
        blocks = [b for b in page_dict.get("blocks", []) if b.get("type") == 0]
        
        # 1. Expand blocks into separate horizontal lines
        lines_on_page = []
        for block in blocks:
            for line in block.get("lines", []):
                bbox = line.get("bbox", [0, 0, 0, 0])
                line_text = ""
                max_size = 0.0
                for span in line.get("spans", []):
                    txt = span.get("text", "")
                    if txt.strip():
                        line_text += txt + " " 
                        size = span.get("size", 0.0)
                        all_sizes.append(round(size, 1))
                        if size > max_size:
                            max_size = size
                
                cleaned = _clean_text(line_text)
                if cleaned:
                    lines_on_page.append({
                        "text": cleaned,
                        "size": round(max_size, 1),
                        "y0": bbox[1],
                        "x0": bbox[0]
                    })
        
        # 2. Cluster lines that are on the same vertical Y-plane
        lines_on_page.sort(key=lambda item: item["y0"])
        
        clustered_rows = []
        current_row = []
        current_y = -1
        
        for item in lines_on_page:
            if current_y == -1 or abs(item["y0"] - current_y) <= 4.0:
                current_row.append(item)
                if current_y == -1:
                    current_y = item["y0"]
            else:
                clustered_rows.append(current_row)
                current_row = [item]
                current_y = item["y0"]
        if current_row:
            clustered_rows.append(current_row)
            
        # 3. For each row, sort left-to-right by X-coordinate
        for row in clustered_rows:
            row.sort(key=lambda item: item["x0"])
            texts = [item["text"].replace("|", "｜") for item in row] # sanitize markdown table pipes
            max_row_size = max([item["size"] for item in row]) if row else 10.0
            
            blocks_data.append({
                "items": texts,
                "size": max_row_size
            })

    if not blocks_data:
        return ""

    # Determine base font size
    size_counts = Counter(all_sizes)
    base_size = size_counts.most_common(1)[0][0] if size_counts else 10.0
    
    # Classify blocks into Markdown with Markdown Table generation
    md_lines = ["\n<div class=\"gov-doc\">\n\n"]
    
    in_table = False
    table_cols = 0
    
    for i, b in enumerate(blocks_data):
        items = b["items"]
        size = b["size"]
        
        # Table detection heuristic:
        # Only true tables usually have 3 or more columns.
        is_multi_col = len(items) >= 3
        
        if is_multi_col:
            if not in_table:
                in_table = True
                table_cols = len(items)
                # print Header
                md_lines.append("| " + " | ".join(items) + " |")
                # print Separator
                md_lines.append("|" + "|".join(["---"] * table_cols) + "|")
            else:
                # pad or truncate items to match table_cols
                if len(items) < table_cols:
                    items.extend([""] * (table_cols - len(items)))
                md_lines.append("| " + " | ".join(items[:table_cols]) + " |")
            continue
        
        if in_table:
            in_table = False
            md_lines.append("\n")
            
        # If len == 2, it's a hanging indent or spaced title. Join with full-width spaces.
        text = "\u3000\u3000".join(items)
        
        # Heuristic rules:
        if size > base_size + 4.0:
            md_lines.append(f"# {text}\n")
        elif size > base_size + 1.5:
            md_lines.append(f"## {text}\n")
        elif size > base_size + 0.1:
            md_lines.append(f"### {text}\n")
        else:
            # Paragraph
            md_lines.append(f"{text}\n")

    md_lines.append("\n</div>\n")
    return "\n".join(md_lines)
