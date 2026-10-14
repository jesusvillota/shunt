import http.server, time, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get('content-length', 0)))
        if 'generate' in self.path.lower() or 'stream' in self.path.lower():
            print(time.strftime('%H:%M:%S'), 'MOCK HIT', self.path, flush=True)
            b = b'{"error":{"code":429,"message":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 15s.","status":"RESOURCE_EXHAUSTED"}}'
            self.send_response(429)
        else:
            b = b'{}'; self.send_response(404)
        self.send_header('content-type', 'application/json'); self.send_header('content-length', str(len(b))); self.end_headers(); self.wfile.write(b)
    do_GET = do_POST
    def log_message(self, *a): pass
http.server.HTTPServer(('127.0.0.1', 39011), H).serve_forever()
