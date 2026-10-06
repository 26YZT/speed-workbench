# 标题工坊：本地启动

## 1. 启动

需要 Python 3.10 或更新版本，无需安装第三方包，也不需要 API 密钥。打开终端：

```sh
cd /path/to/speed-workbench/docs/evidence/L2-2026-10-06/sample
python3 -B app.py
```

保持终端运行，在浏览器打开 **http://127.0.0.1:8080**。不要直接双击 HTML。输入产品关键词，点击“生成三个标题”。页面会显示后端返回的三个标题，并自动保存到历史。生成器为本地固定模板，不是 AI 模型。

## 2. 停止与重启

在终端按 Ctrl-C 停止；重新运行相同命令即可。刷新网页和停止服务都不会清空历史。默认数据文件为本项目目录下的 `data/history.sqlite3`，与终端工作目录无关。备份时先停止服务，然后复制整个 data 目录；不要删除数据库。

## 3. 常见问题

- 端口占用：运行 `python3 -B app.py --port 8081`，打开 http://127.0.0.1:8081。
- 无法连接：确认终端服务仍运行、网址端口正确；恢复服务后点击“刷新历史”或重新提交。
- 历史为空：默认创建本产品的新数据库，不自动导入早期任务目录的数据。可通过 `--database /绝对路径/history.sqlite3` 指定旧数据库，建议先备份。
- 数据库存储错误：检查目录写权限、磁盘空间与数据库文件。不要用删除数据的方式恢复。
- 只监听本机 127.0.0.1；这是开发原型，不适合直接公开到网络。

## 4. 交付结构

运行仅依赖根目录的 `app.py`、`server.py`、`title_history.py` 和 `static/`。`.tasks/` 为任务证据，不是运行依赖；搬运产品时保留上述文件和所需 data 目录即可。不需要单独运行 server.py。

接口：POST /api/generate，GET /api/generations；后者与兼容 GET /api/history 返回相同的 `{ok:true,history:[...]}` 数据。

## 5. 复验

```sh
python3 -B -m unittest discover -s tests -v
node --test tests/ui.test.cjs
node --check static/app.js
```

Node.js 仅运行UI逻辑测试时需要；产品运行只需要Python与浏览器。测试使用临时数据库，不写默认用户历史。完整证据见 ACCEPTANCE.md。
