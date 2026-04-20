import threading

_marker_converter = None
_marker_lock = threading.Lock()
MARKER_AVAILABLE = False

try:
    import marker
    MARKER_AVAILABLE = True
except ImportError:
    pass

def get_marker_converter():
    """
    Lazy loader for Marker PDF Converter and its PyTorch models.
    This guarantees we only allocate VRAM once per process lifetime.
    """
    global _marker_converter
    if not MARKER_AVAILABLE:
        return None
        
    if _marker_converter is None:
        with _marker_lock:
            if _marker_converter is None:
                print(">>> [Marker-PDF] First-time GPU model initialization... (This will consume VRAM)")
                try:
                    # Marker v1.x API
                    from marker.converters.pdf import PdfConverter
                    from marker.models import create_model_dict
                    
                    _marker_converter = PdfConverter(
                        artifact_dict=create_model_dict(),
                    )
                    print(">>> [Marker-PDF] Models loaded successfully into VRAM.")
                except Exception as e:
                    print(f">>> [Marker-PDF] Failed to load deep inference models: {e}")
                    _marker_converter = "FAILED"
                    
    if _marker_converter == "FAILED":
        return None
    return _marker_converter

def extract_pdf_marker(pdf_path: str) -> str:
    """
    Uses the global Marker-PDF converter to parse a single PDF file securely.
    """
    converter = get_marker_converter()
    if not converter:
        raise Exception("Marker converter unavailable")
        
    from marker.output import text_from_rendered
    rendered = converter(pdf_path)
    text, _, _ = text_from_rendered(rendered)
    return text
