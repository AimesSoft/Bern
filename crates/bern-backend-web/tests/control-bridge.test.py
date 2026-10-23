"""Run with python3; no application or browser is needed for relay checks."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import tempfile
import threading
import unittest
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location('bridge', Path(__file__).parents[1]/'templates/serve-bridge.py')
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


class BridgeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='bern-', dir='/tmp')
        self.root = Path(self.tmp.name)
        self.old_dir = os.environ.get('BERN_IPC_DIR')
        os.environ['BERN_IPC_DIR'] = str(self.root/'ipc')
        (self.root/'control.json').write_text('{"bundle_id":"test.web"}')
        (self.root/'index.html').write_text('test app')
        (self.root/'app.wasm').write_bytes(b'0123456789')
        (self.root/'glue').mkdir()
        (self.root/'glue/private.rs').write_text('not a web resource')
        self.server = bridge.Bridge(self.root, 0)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f'http://127.0.0.1:{self.server.server_port}'

    def tearDown(self):
        self.server.shutdown(); self.server.server_close(); self.thread.join()
        if self.old_dir is None: os.environ.pop('BERN_IPC_DIR', None)
        else: os.environ['BERN_IPC_DIR'] = self.old_dir
        self.tmp.cleanup()

    def post(self, action, data, **headers):
        values = {'Content-Type':'application/json','Origin':self.url,
                  'Authorization':'Bearer '+self.server.token}
        values.update(headers)
        req = urllib.request.Request(self.url+'/__bern_bridge/'+action,
                                     data=json.dumps(data).encode(), headers=values)
        with urllib.request.urlopen(req, timeout=3) as response: return json.load(response)

    def test_authentication_origin_and_host_are_required(self):
        for headers, status in [({'Authorization':'Bearer wrong'},401),
                                ({'Origin':'https://other.example'},403),
                                ({'Host':'other.example'},403)]:
            with self.subTest(headers=headers), self.assertRaises(urllib.error.HTTPError) as error:
                self.post('register', {'engine':'engine'}, **headers)
            self.assertEqual(error.exception.code, status)
            error.exception.close()
        self.assertEqual(self.server.pages, {})

    def test_assets_range_and_private_paths(self):
        with urllib.request.urlopen(self.url+'/') as response: self.assertEqual(response.read(),b'test app')
        req=urllib.request.Request(self.url+'/app.wasm',headers={'Range':'bytes=2-5'})
        with urllib.request.urlopen(req) as response:
            self.assertEqual(response.status,206); self.assertEqual(response.read(),b'2345')
            self.assertEqual(response.headers['Content-Type'],'application/wasm')
        for path in ['/control.json','/glue/private.rs','/glue/','/%2e%2e/control.json']:
            with self.subTest(path=path), self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(self.url+path)
            self.assertEqual(error.exception.code,404)
            error.exception.close()

    def test_pages_are_isolated_and_closed_sockets_disappear(self):
        first=self.post('register',{'engine':'first'})['page']
        second=self.post('register',{'engine':'second'})['page']
        page=self.server.pages[first]
        results=[]
        thread=threading.Thread(target=lambda:results.append(page.request('mcp',{'type':'state','request_id':'1'})))
        thread.start()
        task=self.post('next',{'page':first})['tasks'][0]
        self.assertEqual(task['entry'],'mcp')
        with self.assertRaises(urllib.error.HTTPError) as wrong:
            self.post('reply',{'page':second,'id':task['id'],'message':{'type':'state'}})
        self.assertEqual(wrong.exception.code,404)
        wrong.exception.close()
        self.post('reply',{'page':first,'id':task['id'],'message':{'type':'state','revision':'5'}})
        thread.join(2); self.assertEqual(results,[{'type':'state','revision':'5'}])
        self.post('disconnect',{'page':first})
        self.assertFalse(page.path.exists())
        self.assertTrue(self.server.pages[second].path.exists())

    def test_missing_handshake_does_not_dispatch(self):
        page=self.server.pages[self.post('register',{'engine':'first'})['page']]
        with socket.socket(socket.AF_UNIX) as client:
            client.connect(str(page.path)); client.sendall(b'{"type":"state","request_id":"1"}\n')
            result=json.loads(client.recv(4096))
        self.assertEqual(result['error']['code'],'handshake_required')
        self.assertTrue(page.tasks.empty())

    def test_timed_out_queued_commands_are_not_later_executed(self):
        page=self.server.pages[self.post('register',{'engine':'first'})['page']]
        previous=bridge.TIMEOUT; bridge.TIMEOUT=0.01
        try: result=page.request('mcp',{'type':'command','request_id':'1'})
        finally: bridge.TIMEOUT=previous
        self.assertEqual(result['error']['code'],'page_timeout')
        self.assertEqual(page.next(),[])


if __name__=='__main__': unittest.main()
