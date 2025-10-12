"""Simple HTTP server utility for serving content on localhost."""

from __future__ import annotations

from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Optional, Type


def run(port: int, handler: Type[BaseHTTPRequestHandler]) -> None:
    """Run an HTTP server bound to localhost on the specified port."""
    httpd: Optional[HTTPServer] = None
    try:
        httpd = HTTPServer(("localhost", port), handler)
        httpd.serve_forever()
    finally:
        if httpd is not None:
            httpd.server_close()
