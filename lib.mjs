// preload
const url=new URL('lib.wasm',import.meta.url);
await (await fetch(url)).arrayBuffer();
const isWorker=!!globalThis.WorkerGlobalScope&&globalThis instanceof WorkerGlobalScope;
if(isWorker){
  const [hash,n]=globalThis.name.split('-');
  let wasm;
  const {instance}=await WebAssembly.instantiateStreaming(fetch(url,{cache:'force-cache'}),{
    js:{
      println:(ptr,len)=>console.log(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,ptr,len))),
      eprintln:(ptr,len)=>console.error(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,ptr,len)))
    }
  });
  wasm=instance.exports;
  onmessage=async({data})=>{
    if(typeof data==='object'){
      const {hash:h,input}=data;
      if(h===hash&&input instanceof Uint8Array){
        const ptrAndLenPtr=wasm.subset();
        const [ptr,len]=new Uint32Array(wasm.memory.buffer,ptrAndLenPtr,2);
        const output=new Uint8Array(wasm.memory.buffer,ptr,len);
        postMessage({hash,output});
      }
    }
  };
  postMessage({hash,ready:true});
}
/**
 * @param {?AbortSignal} signal
 * @return {Promise<Uint8Array>}
 */
const subset=async(signal)=>{
  const input=new Uint8Array(0);
  const random=crypto.getRandomValues(new Uint8Array(16));
  const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',random)).toHex();
  const worker=await new Promise((resolve,reject)=>{
    const worker=new Worker(import.meta.url,{type:'module',credentials:'omit',name:`${hash}-0`});
    worker.onerror=_=>reject();
    worker.onmessage=({data})=>{
      if(typeof data==='object'){
        const {hash:h,ready}=data;
        if(h===hash&&ready){
          worker.onmessage=null;
          resolve(worker);
        }
      }
      resolve(data);
    }
  });
  if(signal) signal.throwIfAborted();
  return await new Promise((resolve,reject)=>{
    if(signal?.aborted) return reject();
    worker.onerror=_=>{
      worker.terminate();
      reject();
    };
    if(signal) signal.addEventListener('abort',worker.onerror);
    worker.onmessage=({data})=>{
      if(typeof data==='object'){
        const {hash:h,output}=data;
        if(h===hash&&output){
          worker.terminate();
          resolve(output);
        }
      }
    };
    worker.postMessage({hash,input});
  });
};
export {subset};
export default subset;