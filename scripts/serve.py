#!/usr/bin/env python3
"""HTTP server with correct MIME types for .jsx, .yaml, .wasm, etc."""

import http.server
import mimetypes
import sys

# Register MIME types so DevTools shows source instead of hex dump
mimetypes.add_type("application/javascript", ".jsx")
mimetypes.add_type("application/javascript", ".mjs")
mimetypes.add_type("application/wasm", ".wasm")
mimetypes.add_type("text/yaml", ".yaml")
mimetypes.add_type("text/yaml", ".yml")

port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
http.server.test(HandlerClass=http.server.SimpleHTTPRequestHandler, port=port)
