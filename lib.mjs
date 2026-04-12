// preload
const url=new URL('lib.wasm',import.meta.url);
await (await fetch(url)).arrayBuffer();
const isWorker=!!globalThis.WorkerGlobalScope&&globalThis instanceof WorkerGlobalScope;
if(isWorker){
  const hash=globalThis.name;
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
      const {hash:h,input,text,ttf}=data;
      if(h===hash&&input instanceof Uint8Array){
        if(text instanceof Uint8Array){
          const len1=input.length;
          const ptr1=wasm.alloc(len1);
          const len2=text.length;
          const ptr2=wasm.alloc(len2);
          new Uint8Array(wasm.memory.buffer,ptr1,len1).set(input);
          new Uint8Array(wasm.memory.buffer,ptr2,len2).set(text);
          const ptrAndLenPtr=wasm.subset(ptr1,len1,ptr2,len2,!ttf);
          const [ptr3,len3]=new Uint32Array(wasm.memory.buffer,ptrAndLenPtr,2);
          const output=new Uint8Array(wasm.memory.buffer,ptr3,len3);
          postMessage({hash,output});
        }else{
          const len1=input.length;
          const ptr1=wasm.alloc(len1);
          new Uint8Array(wasm.memory.buffer,ptr1,len1).set(input);
          const ptrAndLenPtr=wasm.metadata(ptr1,len1);
          const [ptr2,len2]=new Uint32Array(wasm.memory.buffer,ptrAndLenPtr,2);
          const metadata=new Uint8Array(wasm.memory.buffer,ptr2,len2);
          postMessage({hash,metadata,input},[input.buffer]);
        }
      }
    }
  };
  postMessage({hash,ready:true});
}
/**
 * @param {Uint8Array} input
 * @param {string} text
 * @param {boolean} woff2
 * @param {?AbortSignal} signal
 * @return {Promise<Uint8Array>}
 */
const subset=async(input,text,woff2,signal)=>{
  const random=crypto.getRandomValues(new Uint8Array(16));
  const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',random)).toHex();
  const worker=await new Promise((resolve,reject)=>{
    const worker=new Worker(import.meta.url,{type:'module',credentials:'omit',name:hash});
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
    const encoded=new TextEncoder().encode(text);
    worker.postMessage({hash,input,text:encoded,ttf:!woff2},[input.buffer,encoded.buffer]);
  });
};
/**
 * @typedef {Record<string,any>} Codepoint
 * @property {number} n
 * @property {string} [name]
 */
/**
 * @typedef {Record<string,any>} Axis
 * @property {string} tag
 * @property {number} min
 * @property {number} max
 * @property {number} default
 */
/**
 * @typedef {Record<string,any>} Feature
 * @property {string} tag
 * @property {string} name
 */
/**
 * @typedef {Record<string,any>} Table
 * @property {string} name
 * @property {number} start
 * @property {number} end
 * @property {number[]} codepoints
 * @property {{n:number,name:string}[]} codepoint_names
 */
/**
 * @typedef {Record<string,any>} Metadata
 * @property {Uint8Array} input
 * @property {string} family_name
 * @property {number} codepoint_count
 * @property {Feature[]} features
 * @property {Axis[]} axes
 * @property {Table[]} tables
 */
/**
 * @param {Uint8Array} input
 * @param {?AbortSignal} signal
 * @return {Promise<Metadata>}
 */
const metadata=async(input,signal)=>{
  const random=crypto.getRandomValues(new Uint8Array(16));
  const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',random)).toHex();
  const worker=await new Promise((resolve,reject)=>{
    const worker=new Worker(import.meta.url,{type:'module',credentials:'omit',name:hash});
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
        const {hash:h,metadata,input}=data;
        if(h===hash&&metadata&&input){
          worker.terminate();
          const json=new TextDecoder().decode(metadata);
          const result=JSON.parse(json);
          result.input=input;
          resolve(result);
        }
      }
    };
    worker.postMessage({hash,input},[input.buffer]);
  });
};
export {subset,metadata};
