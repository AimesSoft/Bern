#!/usr/bin/env python3
"""Serve a pure Web app and relay its command catalog to local CLI/MCP clients.

No engine runs here. Each authenticated page owns an IPC socket; every request
is forwarded to that page and is answered only by its WASM runtime.
"""
import argparse
import hmac
import json
import os
from pathlib import Path
import queue
import re
import secrets
import socket
import stat
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import unquote, urlsplit, urlencode

MAX_BYTES = 1 << 20
TIMEOUT = 30


def ipc_directory(bundle):
    fingerprint = 0xcbf29ce484222325
    for byte in bundle.encode():
        fingerprint = ((fingerprint ^ byte) * 0x100000001b3) & ((1 << 64)-1)
    root = Path(os.environ.get('BERN_IPC_DIR') or os.environ.get('TMPDIR') or tempfile.gettempdir())
    directory = root / f'bern-{fingerprint:016x}'
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or (hasattr(os, 'getuid') and info.st_uid != os.getuid()):
        raise PermissionError('IPC directory must belong to this user')
    directory.chmod(0o700)
    return directory


def error(code, message, request=None):
    return {'type':'error', 'request_id':request, 'error':{'code':code, 'message':message}}


class Pending:
    def __init__(self, entry, message):
        self.id = secrets.token_hex(16)
        self.entry, self.message = entry, message
        self.ready = threading.Event()
        self.result = None


class Page:
    def __init__(self, directory, engine):
        self.id = secrets.token_hex(24)
        self.engine = engine
        self.path = directory / f'{engine}.sock'
        self.tasks = queue.Queue(maxsize=128)
        self.pending = {}
        self.connections = set()
        self.lock = threading.Lock()
        self.last_seen = time.monotonic()
        self.closed = False
        self.listener = socket.socket(socket.AF_UNIX)
        # Do not unlink or replace another live instance's socket.
        self.listener.bind(str(self.path))
        self.path.chmod(0o600)
        self.listener.listen(16)
        self.listener.settimeout(1)
        threading.Thread(target=self.accept, daemon=True).start()

    def accept(self):
        while not self.closed:
            try: stream, _ = self.listener.accept()
            except socket.timeout: continue
            except OSError: break
            with self.lock:
                if self.closed or len(self.connections) >= 32:
                    stream.close(); continue
                self.connections.add(stream)
            threading.Thread(target=self.client, args=(stream,), daemon=True).start()

    def request(self, entry, message):
        pending = Pending(entry, message)
        with self.lock:
            if self.closed:
                return error('page_disconnected', 'the browser page has closed', message.get('request_id'))
            self.pending[pending.id] = pending
            try: self.tasks.put_nowait(pending)
            except queue.Full:
                del self.pending[pending.id]
                return error('bridge_busy', 'too many pending requests', message.get('request_id'))
        answered = pending.ready.wait(TIMEOUT)
        with self.lock: self.pending.pop(pending.id, None)
        if not answered:
            return error('page_timeout', 'the page did not answer; outcome is unknown, do not retry automatically', message.get('request_id'))
        return pending.result

    def client(self, stream):
        try:
            stream.settimeout(TIMEOUT)
            reader = stream.makefile('rb')
            entry, greeted = 'json', False
            with reader:
                while not self.closed:
                    line = reader.readline(MAX_BYTES+1)
                    if not line: break
                    if len(line) > MAX_BYTES: break
                    try:
                        message = json.loads(line)
                        if not isinstance(message, dict): raise ValueError('expected an object')
                    except ValueError:
                        result = error('invalid_message', 'invalid JSON control message')
                    else:
                        if not greeted and message.get('type') != 'hello':
                            result = error('handshake_required', 'send hello first')
                        elif greeted and message.get('type') == 'hello':
                            result = error('invalid_message', 'already connected')
                        else:
                            if not greeted: entry = message.get('entry', 'json')
                            result = self.request(entry, message)
                            if result.get('type') == 'welcome':
                                greeted = True
                                stream.settimeout(None)
                    stream.sendall(json.dumps(result, separators=(',',':')).encode()+b'\n')
                    if not greeted: break
        except (OSError, ValueError): pass
        finally:
            with self.lock: self.connections.discard(stream)
            stream.close()

    def next(self):
        self.last_seen = time.monotonic()
        try: pending = self.tasks.get(timeout=20)
        except queue.Empty: return []
        with self.lock:
            if pending.id not in self.pending: return []
        return [{'id':pending.id, 'entry':pending.entry, 'message':pending.message}]

    def reply(self, task_id, message):
        with self.lock:
            pending = self.pending.get(task_id)
            if not pending or pending.ready.is_set(): return False
            pending.result = message
            pending.ready.set()
            return True

    def close(self):
        with self.lock:
            if self.closed: return
            self.closed = True
            for pending in self.pending.values():
                pending.result = error('page_disconnected', 'the browser page disconnected; outcome may be unknown', pending.message.get('request_id'))
                pending.ready.set()
            for stream in self.connections:
                try: stream.shutdown(socket.SHUT_RDWR)
                except OSError: pass
            self.listener.close()
            self.path.unlink(missing_ok=True)


class Bridge(ThreadingHTTPServer):
    daemon_threads = True
    def __init__(self, root, port):
        self.root = Path(root).resolve()
        config = json.loads((self.root / 'control.json').read_text())
        self.directory = ipc_directory(config['bundle_id'])
        self.token = secrets.token_urlsafe(32)
        self.pages, self.lock = {}, threading.Lock()
        super().__init__(('127.0.0.1', port), Handler)
        self.stopping = threading.Event()
        threading.Thread(target=self.reap, daemon=True).start()

    def reap(self):
        while not self.stopping.wait(5):
            with self.lock:
                for page_id, page in list(self.pages.items()):
                    if time.monotonic()-page.last_seen > 60:
                        page.close(); del self.pages[page_id]

    def server_close(self):
        self.stopping.set()
        with self.lock:
            for page in self.pages.values(): page.close()
            self.pages.clear()
        super().server_close()


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(args[2].root), **kwargs)

    def log_message(self, *_): pass

    def host_ok(self):
        return self.headers.get('Host', '').lower() in {
            f'127.0.0.1:{self.server.server_port}', f'localhost:{self.server.server_port}'}

    def json(self, value, status=200):
        data = json.dumps(value, separators=(',',':')).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if not self.host_ok(): return self.json({}, 403)
        origin = self.headers.get('Origin')
        if origin != 'http://' + self.headers.get('Host', ''): return self.json({}, 403)
        if not hmac.compare_digest(self.headers.get('Authorization', ''), 'Bearer '+self.server.token):
            return self.json({}, 401)
        if self.headers.get_content_type() != 'application/json': return self.json({}, 415)
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= MAX_BYTES: return self.json({}, 413)
            self.connection.settimeout(30)
            data = json.loads(self.rfile.read(length))
            if not isinstance(data, dict): return self.json({}, 400)
            action = self.path.removeprefix('/__bern_bridge/')
            if action == 'register':
                engine = data.get('engine', '')
                if not isinstance(engine, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,64}', engine):
                    return self.json({}, 400)
                with self.server.lock:
                    if len(self.server.pages) >= 16: return self.json({}, 429)
                    page = Page(self.server.directory, engine)
                    self.server.pages[page.id] = page
                return self.json({'page':page.id})
            with self.server.lock: page = self.server.pages.get(data.get('page'))
            if page is None: return self.json({}, 404)
            if action == 'next': return self.json({'tasks':page.next()})
            if action == 'reply':
                message = data.get('message')
                if not isinstance(message, dict): return self.json({}, 400)
                return self.json({}, 200 if page.reply(data.get('id'), message) else 404)
            if action == 'disconnect':
                with self.server.lock:
                    self.server.pages.pop(page.id, None)
                page.close()
                return self.json({})
            return self.json({}, 404)
        except (ValueError, TypeError): self.json({}, 400)
        except OSError: self.json({}, 409)

    def list_directory(self, path):
        self.send_error(404); return None

    def send_head(self):
        self.range_left = None
        if not self.host_ok(): self.send_error(403); return None
        route = unquote(urlsplit(self.path).path)
        if any(part in {'glue','bin','skills','package','test-fixtures','..'} or part.startswith('.') for part in route.split('/') if part):
            self.send_error(404); return None
        path = Path(self.translate_path(self.path))
        try: path.resolve().relative_to(self.server.root)
        except ValueError: self.send_error(404); return None
        if route in {'/control.json','/serve-bridge.py'}: self.send_error(404); return None
        header = self.headers.get('Range')
        if not header or not path.is_file(): return super().send_head()
        match = re.fullmatch(r'bytes=(\d+)-(\d*)', header)
        size = path.stat().st_size
        if not match: self.send_error(416); return None
        start = int(match[1]); end = min(size-1, int(match[2]) if match[2] else size-1)
        if start > end: self.send_error(416); return None
        file = path.open('rb'); file.seek(start)
        self.range_left = end-start+1
        self.send_response(206)
        self.send_header('Content-Type', self.guess_type(str(path)))
        self.send_header('Content-Length', str(self.range_left))
        self.send_header('Content-Range', f'bytes {start}-{end}/{size}')
        self.send_header('Accept-Ranges', 'bytes')
        self.end_headers()
        return file

    def end_headers(self):
        self.send_header('Cache-Control', 'no-cache')
        super().end_headers()

    def copyfile(self, source, output):
        if self.range_left is None: return super().copyfile(source, output)
        left = self.range_left
        while left:
            data = source.read(min(left, 65536))
            if not data: break
            output.write(data); left -= len(data)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', default=str(Path(__file__).resolve().parent))
    parser.add_argument('--port', type=int, default=0)
    parser.add_argument('--trace')
    args = parser.parse_args()
    server = Bridge(args.directory, args.port)
    fragment = {'bern-bridge':server.token}
    if args.trace: fragment['bern-trace'] = args.trace
    print(f'open http://127.0.0.1:{server.server_port}/#{urlencode(fragment)}', flush=True)
    try: server.serve_forever()
    except KeyboardInterrupt: pass
    finally: server.server_close()
