"""Package-only smoke: real processes, restart, persistence and recovery."""
import json
import selectors
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.error import HTTPError

ROOT = Path(__file__).resolve().parents[1]

class RestartSmoke(unittest.TestCase):
    def test_standalone_process_restart_and_error_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            package=Path(directory)/'product'
            package.mkdir()
            for name in ['app.py','server.py','title_history.py']:
                shutil.copy2(ROOT/name,package/name)
            shutil.copytree(ROOT/'static',package/'static')
            def start():
                process=subprocess.Popen([sys.executable,'-B',str(package/'app.py'),'--port','0'],cwd=directory,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                selector=selectors.DefaultSelector()
                selector.register(process.stdout,selectors.EVENT_READ)
                try:
                    if not selector.select(10):
                        process.kill(); process.communicate(timeout=5)
                        self.fail('server startup timeout')
                    line=process.stdout.readline().strip()
                    if not line.startswith('Open http://127.0.0.1:'):
                        process.kill(); _,err=process.communicate(timeout=5)
                        self.fail('server startup failed: '+line+err)
                    return process,line.removeprefix('Open ')
                finally:
                    selector.close()
            def stop(process):
                process.terminate()
                try: process.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill();process.communicate(timeout=5)
            def request(base,route,payload=None):
                req=Request(base+route,data=None if payload is None else json.dumps(payload).encode(),headers={'Content-Type':'application/json'})
                try:
                    with urlopen(req,timeout=5) as response:return response.status,json.load(response)
                except HTTPError as error:
                    with error:return error.code,json.load(error)
            process,base=start()
            try:
                with urlopen(base,timeout=5) as response:self.assertIn('标题工坊',response.read().decode())
                self.assertEqual(request(base,'/api/generate',{})[0],400)
                status,result=request(base,'/api/generate',{'product_idea':'重启验收产品'})
                self.assertEqual(status,200)
                record=result['result']
                self.assertEqual(len(record['titles']),3)
                for _ in range(2): self.assertEqual(request(base,'/api/generations')[1]['history'],[record])
            finally:stop(process)
            database=package/'data'/'history.sqlite3'
            self.assertTrue(database.exists())
            process,base=start()
            try:
                self.assertEqual(request(base,'/api/generations')[1]['history'],[record])
                with sqlite3.connect(database) as db: db.execute('ALTER TABLE title_history RENAME TO history_backup')
                self.assertEqual(request(base,'/api/generations')[0],503)
                self.assertEqual(request(base,'/api/generate',{'product_idea':'失败不入库'})[0],503)
                with sqlite3.connect(database) as db: db.execute('ALTER TABLE history_backup RENAME TO title_history')
                self.assertEqual(request(base,'/api/generations')[1]['history'],[record])
                self.assertEqual(request(base,'/api/generate',{'product_idea':'恢复后产品'})[0],200)
                self.assertEqual(len(request(base,'/api/generations')[1]['history']),2)
            finally:stop(process)
