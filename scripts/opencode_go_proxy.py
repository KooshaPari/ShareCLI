#!/usr/bin/env python3
"""
opencode_go_proxy.py — local translation proxy for Codex CLI → OpenCode Go.

Why this exists
---------------
Codex CLI requires `wire_api = "responses"` on its model_providers config.
OpenCode Go's /v1/responses endpoint is the same as /v1/chat/completions but
requires an `x-opencode-session` header (which codex does not add). Without
the header, OpenCode Go returns MissingSessionID and codex silently hangs.

This proxy listens on http://127.0.0.1:20129 and:
  - Accepts POST /v1/responses (codex-style)
  - Translates the request body to OpenAI chat/completions format
  - Adds the x-opencode-session header
  - Forwards to https://opencode.ai/zen/go/v1/chat/completions
  - Translates the response back to responses format
  - Returns it to codex

Run:
  OPENCODE_GO_API_KEY=sk-... python3 opencode_go_proxy.py

Then in codex profile:
  base_url = "http://127.0.0.1:20129/v1"
  wire_api = "responses"
"""

import json
import os
import sys
import time
import urllib.request
import urllib.error
from http.server import BaseHTTPRequestHandler, HTTPServer

UPSTREAM = os.environ.get("OPENCODE_GO_UPSTREAM", "https://opencode.ai/zen/go/v1")
API_KEY = os.environ.get("OPENCODE_GO_API_KEY", "")
SESSION = os.environ.get("OPENCODE_GO_SESSION", f"codex-proxy-{os.getpid()}-{int(time.time())}")
LISTEN_HOST = os.environ.get("PROXY_HOST", "127.0.0.1")
LISTEN_PORT = int(os.environ.get("PROXY_PORT", "20129"))

if not API_KEY:
    print("OPENCODE_GO_API_KEY is required", file=sys.stderr)
    sys.exit(2)


def translate_responses_to_chat(body: dict) -> dict:
    """Translate Codex /v1/responses request → OpenAI chat/completions.

    Codex responses API uses {model, input, instructions, ...}.
    OpenAI chat/completions uses {model, messages, ...}.
    """
    model = body.get("model", "deepseek-v4.1-flash")
    messages = []

    instructions = body.get("instructions")
    if instructions:
        messages.append({"role": "system", "content": instructions})

    inp = body.get("input")
    if isinstance(inp, str):
        messages.append({"role": "user", "content": inp})
    elif isinstance(inp, list):
        # Responses API items: {type: "message", role, content: [{type, text}]}
        for item in inp:
            if not isinstance(item, dict):
                continue
            role = item.get("role") or "user"
            content = item.get("content")
            if isinstance(content, list):
                text_parts = [
                    p.get("text", "") for p in content if isinstance(p, dict) and p.get("type") == "text"
                ]
                content = "\n".join(text_parts)
            if content:
                messages.append({"role": role, "content": content})

    chat = {
        "model": model,
        "messages": messages,
        "stream": False,
    }
    # Carry over temperature, max_tokens, top_p if present
    for k in ("temperature", "max_tokens", "top_p"):
        if k in body:
            chat[k] = body[k]
    return chat


def translate_chat_to_responses(chat_resp: dict, model: str) -> dict:
    """Translate OpenAI chat/completions response → Codex responses API."""
    choices = chat_resp.get("choices", [])
    text = ""
    finish = "stop"
    if choices:
        msg = choices[0].get("message", {})
        text = msg.get("content", "")
        finish = choices[0].get("finish_reason", "stop") or "stop"

    return {
        "id": chat_resp.get("id", f"resp_{int(time.time()*1000)}"),
        "object": "response",
        "created": chat_resp.get("created", int(time.time())),
        "model": chat_resp.get("model", model),
        "output": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text}],
            }
        ],
        "usage": chat_resp.get("usage", {}),
        "status": "completed",
        "finish_reason": finish,
    }


class ProxyHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        # Quieter logging — print only requests, not full headers
        sys.stderr.write("[proxy] " + (fmt % args) + "\n")

    def _send_json(self, status: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_error_json(self, status: int, message: str):
        self._send_json(status, {"error": {"type": "proxy_error", "message": message}})

    def do_GET(self):
        if self.path == "/health":
            self._send_json(200, {"ok": True, "upstream": UPSTREAM, "session": SESSION})
            return
        self._send_error_json(404, f"GET {self.path} not handled (only POST /v1/responses)")

    def do_POST(self):
        if not self.path.endswith("/responses") and self.path != "/v1/responses":
            self._send_error_json(404, f"POST {self.path} not handled (expected /v1/responses)")
            return

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        try:
            body = json.loads(raw.decode("utf-8"))
        except Exception as exc:
            self._send_error_json(400, f"invalid json: {exc}")
            return

        chat = translate_responses_to_chat(body)
        chat_body = json.dumps(chat).encode("utf-8")

        req = urllib.request.Request(
            f"{UPSTREAM}/chat/completions",
            data=chat_body,
            method="POST",
            headers={
                "Authorization": f"Bearer {API_KEY}",
                "Content-Type": "application/json",
                "x-opencode-session": SESSION,
                "User-Agent": "opencode-go-proxy/1.0 (+codex-cli-bridge)",
                "Accept": "application/json",
            },
        )

        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                upstream_status = resp.status
                upstream_body = resp.read()
        except urllib.error.HTTPError as e:
            err_body = e.read().decode("utf-8", errors="replace")
            self._send_error_json(e.code, f"upstream {e.code}: {err_body[:500]}")
            return
        except urllib.error.URLError as e:
            self._send_error_json(502, f"upstream unreachable: {e.reason}")
            return
        except Exception as e:
            self._send_error_json(500, f"upstream error: {e}")
            return

        try:
            chat_resp = json.loads(upstream_body.decode("utf-8"))
            responses = translate_chat_to_responses(chat_resp, chat["model"])
        except Exception as e:
            self._send_error_json(502, f"bad upstream body: {e}")
            return

        self._send_json(upstream_status, responses)


def main():
    server = HTTPServer((LISTEN_HOST, LISTEN_PORT), ProxyHandler)
    print(f"[proxy] listening on http://{LISTEN_HOST}:{LISTEN_PORT} → {UPSTREAM}", flush=True)
    print(f"[proxy] session id: {SESSION}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[proxy] shutting down", flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
