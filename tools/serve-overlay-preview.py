"""Run a LAN-accessible synthetic overlay preview, never a real DB attachment."""
import argparse
import ipaddress
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from http.client import HTTPConnection

PUBLIC = ''
BACKEND = ''
BACKEND_PORT = 0
HOP = {'connection', 'keep-alive', 'proxy-authenticate', 'proxy-authorization',
       'te', 'trailer', 'transfer-encoding', 'upgrade'}

class Preview(BaseHTTPRequestHandler):
    def forward(self):
        if self.headers.get('Host') != PUBLIC:
            self.send_error(403, 'Unexpected preview host')
            return
        origin = self.headers.get('Origin')
        if origin and origin != 'http://' + PUBLIC:
            self.send_error(403, 'Unexpected preview origin')
            return
        if not self.path.startswith('/') or self.path.startswith('//') or self.headers.get('Transfer-Encoding'):
            self.send_error(400)
            return
        try:
            length = int(self.headers.get('Content-Length', '0'))
        except ValueError:
            self.send_error(400)
            return
        if not 0 <= length <= 1048576:
            self.send_error(413)
            return
        headers = {k: v for k, v in self.headers.items() if k.lower() not in HOP | {'host', 'origin'}}
        headers['Host'] = BACKEND
        if origin:
            headers['Origin'] = 'http://' + BACKEND
        connection = HTTPConnection('127.0.0.1', BACKEND_PORT, timeout=15)
        try:
            connection.request(self.command, self.path, body=self.rfile.read(length), headers=headers)
            response = connection.getresponse()
            body = response.read()
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in HOP | {'content-length'}:
                    self.send_header(key, value.replace('http://' + BACKEND, 'http://' + PUBLIC) if key.lower() == 'location' else value)
            self.send_header('Content-Length', str(len(body)))
            self.send_header('X-Volmap-Preview', 'synthetic-fixture')
            self.end_headers()
            if self.command != 'HEAD':
                self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass
        except (OSError, TimeoutError):
            self.send_error(502, 'Synthetic preview backend unavailable')
        finally:
            connection.close()
    do_GET = do_HEAD = do_POST = forward

def main():
    global PUBLIC, BACKEND, BACKEND_PORT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--listen', type=ipaddress.IPv4Address, default='192.168.4.2')
    parser.add_argument('--port', type=int, default=7777)
    args = parser.parse_args()
    if args.listen.is_unspecified or not 1 <= args.port <= 65535:
        parser.error('Use an explicit IPv4 address and port 1–65535')
    PUBLIC = f'{args.listen}:{args.port}'
    # Reserve the requested public port before spending time building fixtures.
    server = ThreadingHTTPServer((str(args.listen), args.port), Preview)
    server.daemon_threads = True
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        BACKEND_PORT = reservation.getsockname()[1]
    BACKEND = f'127.0.0.1:{BACKEND_PORT}'
    env = os.environ.copy()
    env.update(VOLMAP_BROWSER_PORT=str(BACKEND_PORT), VOLMAP_BROWSER_PRODUCER='1',
               VOLMAP_BROWSER_DENSE='1', VOLMAP_BROWSER_RELEASE='1')
    # The launcher controls only its own synthetic producer and fixture files.
    env.pop('VOLMAP_BROWSER_PRODUCER_PID_FILE', None)
    root = Path(__file__).resolve().parent.parent
    child = None
    def stop(signum, frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, stop)
    try:
        child = subprocess.Popen(['bash', 'release/run-browser-server.sh'],
                                 cwd=root, env=env, start_new_session=True)
        deadline = time.monotonic() + 300
        while True:
            if child.poll() is not None:
                raise RuntimeError(f'Fixture server exited with status {child.returncode}')
            try:
                with socket.create_connection(('127.0.0.1', BACKEND_PORT), timeout=0.2):
                    break
            except OSError:
                if time.monotonic() >= deadline:
                    raise TimeoutError('Fixture server startup exceeded 300 seconds')
                time.sleep(0.2)
        print(f'Synthetic overlay preview: http://{PUBLIC}/volume/0', flush=True)
        print('Click Enable observations. Ctrl-C stops the preview and its fixture producer.', flush=True)
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()

if __name__ == '__main__':
    main()

