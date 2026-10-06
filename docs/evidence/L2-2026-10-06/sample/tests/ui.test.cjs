const test=require('node:test'); const assert=require('node:assert/strict'); const vm=require('node:vm'); const fs=require('node:fs');
const source=fs.readFileSync('static/app.js','utf8');
function harness(fetch){
 const nodes=new Map();
 function element(){return {textContent:'',value:'',hidden:false,disabled:false,children:[],handlers:{},append(...x){this.children.push(...x)},replaceChildren(...x){this.children=x},setAttribute(){},focus(){},addEventListener(k,fn){this.handlers[k]=fn}}}
 const get=id=>{if(!nodes.has(id))nodes.set(id,element());return nodes.get(id)};
 vm.runInNewContext(source,{document:{getElementById:get,createElement:element},fetch,AbortController,setTimeout,clearTimeout,Date});
 return {get,submit:()=>get('generate-form').handlers.submit({preventDefault(){}})};
}
const ok=data=>({ok:true,status:200,json:async()=>({ok:true,...data})});
const record={product_idea:'timer',created_at:'2026-10-06T00:00:00Z',titles:['real one','real two','real three']};
const flush=()=>new Promise(r=>setImmediate(r));
test('empty input errors without POST',async()=>{let posts=0;const h=harness(async(u,o)=>{if(o.method)posts++;return ok({history:[]})});await h.submit();assert.match(h.get('error').textContent,/请输入/);assert.equal(posts,0)});
test('real response rendered, pending submit locked, histories use generations',async()=>{let release;let posts=0;let histories=0;const h=harness(async(u,o)=>{if(u==='/api/generations'){histories++;return ok({history:[record]})}posts++;return new Promise(r=>release=r)});await flush();h.get('idea').value='timer';const pending=h.submit();assert.equal(h.get('generate').disabled,true);await h.submit();assert.equal(posts,1);release(ok({result:record}));await pending;assert.equal(h.get('generate').disabled,false);assert.deepEqual(Array.from(h.get('titles').children,x=>x.textContent),record.titles);assert.equal(h.get('history').children.length,1);assert.ok(histories>=2)});
for(const status of [400,503])test('HTTP '+status+' displays error and unlocks',async()=>{const h=harness(async(u)=>u==='/api/generations'?ok({history:[]}):{ok:false,status,json:async()=>({ok:false,error:{message:'test error'}})});h.get('idea').value='timer';await h.submit();assert.match(h.get('error').textContent,new RegExp(String(status)));assert.equal(h.get('generate').disabled,false)});
test('network failure and history retry visible',async()=>{const h=harness(async()=>{throw new TypeError('offline')});await flush();assert.match(h.get('history-error').textContent,/历史读取失败/);h.get('idea').value='timer';await h.submit();assert.match(h.get('error').textContent,/无法连接/);assert.equal(h.get('generate').disabled,false)});

test('network failure recovers after retry without reload',async()=>{let offline=true;const h=harness(async(u)=>{if(offline)throw new TypeError('offline');return u==='/api/generations'?ok({history:[record]}):ok({result:record})});await flush();h.get('idea').value='timer';await h.submit();assert.match(h.get('error').textContent,/无法连接/);offline=false;await h.submit();assert.equal(h.get('error').hidden,true);assert.equal(h.get('result').hidden,false);assert.equal(h.get('generate').disabled,false);assert.equal(h.get('history').children.length,1)});
test('history failure does not erase successful generated titles',async()=>{const h=harness(async(u)=>{if(u==='/api/generations')throw new TypeError('offline');return ok({result:record})});h.get('idea').value='timer';await h.submit();assert.equal(h.get('result').hidden,false);assert.match(h.get('status').textContent,/生成成功/);assert.match(h.get('history-error').textContent,/历史读取失败/);assert.equal(h.get('generate').disabled,false)});
