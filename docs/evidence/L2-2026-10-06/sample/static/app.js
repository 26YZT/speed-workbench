"use strict";
const $ = id => document.getElementById(id);
let busy = false;
let historyVersion = 0;
function message(id, text) { $(id).textContent = text; $(id).hidden = !text; }
async function request(url, options = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 15000);
  try {
    let response;
    try { response = await fetch(url, {...options, signal: controller.signal}); }
    catch (error) { throw new Error(error.name === 'AbortError' ? '请求超时，请检查本地服务后重试。' : '无法连接本地服务，请确认服务已启动后重试。'); }
    let data;
    try { data = await response.json(); }
    catch { throw new Error('服务返回了无法读取的数据（HTTP ' + response.status + '）。'); }
    if (!response.ok || !data.ok) throw new Error('请求失败（HTTP ' + response.status + '）：' + (data.error?.message || '请稍后重试。'));
    return data;
  } finally { clearTimeout(timer); }
}
function titleList(titles) {
  if (!Array.isArray(titles) || titles.length !== 3 || titles.some(t => typeof t !== 'string')) throw new Error('服务返回的标题格式不正确，请重试。');
  return titles.map(title => { const li = document.createElement('li'); li.textContent = title; return li; });
}
async function loadHistory() {
  const version = ++historyVersion;
  $('refresh').disabled = true;
  message('history-error', '');
  $('history-status').textContent = '正在读取历史…';
  try {
    const data = await request('/api/generations');
    if (version !== historyVersion) return;
    if (!Array.isArray(data.history)) throw new Error('历史记录格式不正确。');
    const entries = data.history.map(record => {
      const article = document.createElement('article'); article.className = 'entry';
      const heading = document.createElement('h3'); heading.textContent = record.product_idea;
      const time = document.createElement('time'); time.dateTime = record.created_at; time.textContent = new Date(record.created_at).toLocaleString('zh-CN');
      const list = document.createElement('ol'); list.append(...titleList(record.titles));
      article.append(heading, time, list); return article;
    });
    $('history').replaceChildren(...entries);
    $('history-status').textContent = entries.length ? '已读取 ' + entries.length + ' 条记录' : '还没有记录，试着生成第一组标题吧。';
  } catch (error) {
    if (version !== historyVersion) return;
    $('history-status').textContent = '';
    message('history-error', '历史读取失败：' + error.message + ' 可点击“刷新历史”重试。');
  } finally { if (version === historyVersion) $('refresh').disabled = false; }
}
$('generate-form').addEventListener('submit', async event => {
  event.preventDefault();
  if (busy) return;
  message('error', ''); $('status').textContent = ''; $('result').hidden = true;
  const idea = $('idea').value.trim();
  if (!idea) { message('error', '请输入产品关键词或想法，再生成标题。'); $('idea').focus(); return; }
  busy = true; $('generate').disabled = true; $('generate').textContent = '正在生成…'; $('generate-form').setAttribute('aria-busy', 'true'); $('status').textContent = '正在生成并保存，请稍候…';
  try {
    const data = await request('/api/generate', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({product_idea:idea})});
    $('titles').replaceChildren(...titleList(data.result?.titles)); $('result').hidden = false;
    $('status').textContent = '生成成功，三个标题已保存。';
    await loadHistory();
  } catch (error) { $('status').textContent = ''; message('error', error.message); }
  finally { busy = false; $('generate').disabled = false; $('generate').textContent = '生成三个标题'; $('generate-form').setAttribute('aria-busy', 'false'); }
});
$('refresh').addEventListener('click', loadHistory);
loadHistory();
