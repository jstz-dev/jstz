async function o(e){if(Kv.get("account0")==="0")Kv.set("account0",Kv.get("account1")),Kv.set("account1","0");else if(Kv.get("account1")==="0")Kv.set("account1",Kv.get("account0")),Kv.set("account0","0");else throw new Error("Invalid account state");return new Response}async function r(e){if(Kv.set("last_sent",(parseInt(Kv.get("last_sent"))+1).toString()),Kv.get("account1")>0)Kv.set("account1",(parseInt(Kv.get("account1"))-1).toString());else throw new Error("Account 1 has no funds");let t="account"+Kv.get("last_sent");return Kv.get(t)===void 0?Kv.set(t,1):Kv.set(t,(parseInt(Kv.get(t))+1).toString()),new Response}async function n(e,t){for(let c=1;c<e+1;c++)Kv.set(`value${c}`,(parseInt(Kv.get(`value${c-1}`))+1).toString());return new Response}async function u(e){let t=Kv.get("smartFunctionAddress"),c=new Response;for(let a=0;a<200;a++)c=await fetch(`jstz://${t}/`,{method:"POST",body:JSON.stringify({message:"hello"})});return c}function s(e){if(!e)throw"Assertion failed"}async function i(e){switch(new URL(e.url).pathname){case"/init_1":return Kv.set("account0","0"),Kv.set("account1","470"),new Response("Success!");case"/init_2":return Kv.set("last_sent","0"),Kv.set("account1","470"),new Response("Success!");case"/init_3":return Kv.set("value0","47"),new Response("Success!");case"/init_4":let a=await SmartFunction.create(`
        async function handler(r){
        if (Kv.get("account0") === undefined) {
          Kv.set("account0", "0");
        }
        if (Kv.get("account1") === undefined) {
          Kv.set("account1", "470");
        }
        if (Kv.get("account0") === "0") {
          Kv.set("account0", Kv.get("account1"));
          Kv.set("account1", "0");
        } else if (Kv.get("account1") === "0") {
          Kv.set("account1", Kv.get("account0"));
          Kv.set("account0", "0");
        } else {
          throw new Error("Invalid account state");
        } return new Response()
        }
        export{ handler as default};
      }`);return Kv.set("smartFunctionAddress",a),new Response("Success!");case"/benchmark_transaction1":return o(e);case"/benchmark_transaction2":return r(e);case"/benchmark_transaction3":return n(5e3,e);case"/benchmark_transaction4":return u(e);case"/benchmark_transaction5":return n(0,e);case"/benchmark_transaction6":return n(1,e);case"/benchmark_transaction7":return n(10,e);case"/benchmark_transaction8":return n(100,e);case"/benchmark_transaction9":return n(1e3,e);case"/benchmark_transaction10":return n(2e3,e);case"/benchmark_transaction11":return n(5e3,e);case"/benchmark_transaction12":return n(1e4,e);case"/benchmark_transaction13":return n(2e4,e);case"/benchmark_transaction14":return n(5e4,e);case"/benchmark_transaction15":return n(1e5,e);case"/check_1":return console.log("Checking..."),s(Kv.get("account0")==="0"),s(Kv.get("account1")==="470"),console.log("Checks succeeded."),new Response("Success!");case"/check_2":return console.log("Checking..."),s(parseInt(Kv.get("last_sent"))>0),s(parseInt(Kv.get("account1"))<470),console.log("Checks succeeded."),new Response("Success!");case"/check_3":return console.log("Checking..."),s(parseInt(Kv.get("value4999"))>0),console.log("Checks succeeded."),new Response("Success!");case"/check_4":return console.log("Checking..."),s(!1),console.log("Checks succeeded."),new Response("Success!");case"/check_5":return console.log("Checking..."),Kv.get("value1")!=null&&(console.log(Kv.get("value1")),s(parseInt(Kv.get("value1"))>0)),console.log("Checks succeeded."),new Response("Success!")}return new Response("Unrecognized entrypoint",{status:404})}var l=i;export{l as default};
