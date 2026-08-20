// Local test-page server used for RAM benchmarking (avoids the sandbox's
// TLS-intercepting proxy — Chrome connects to localhost directly).
const PORT = Number(process.env.TESTPAGES_PORT ?? 8055);

const simple = `<!doctype html><title>Simple page</title>
<h1>Hello from the simple page</h1><p id="msg">benchmark target</p>`;

// A moderately heavy page: 500 DOM nodes, a canvas render, some JS work.
const heavy = `<!doctype html><title>Heavy page</title>
<style>div.b{width:40px;height:40px;display:inline-block;margin:2px;border-radius:8px}</style>
<h1>Heavy page</h1><canvas id=c width=1200 height=600></canvas><div id=grid></div>
<script>
const grid=document.getElementById("grid");
for(let i=0;i<500;i++){const d=document.createElement("div");d.className="b";
  d.style.background="hsl("+(i*7%360)+",70%,60%)";d.textContent=i;grid.append(d);}
const ctx=document.getElementById("c").getContext("2d");
for(let i=0;i<2000;i++){ctx.fillStyle="hsl("+(i%360)+",80%,50%)";
  ctx.beginPath();ctx.arc(Math.sin(i)*600+600,Math.cos(i*1.3)*300+300,20,0,7);ctx.fill();}
window.bigArray = new Array(100000).fill(0).map((_,i)=>({i, s:"item-"+i}));
</script>`;

Bun.serve({
  port: PORT,
  routes: {
    "/simple": () => new Response(simple, { headers: { "Content-Type": "text/html" } }),
    "/heavy": () => new Response(heavy, { headers: { "Content-Type": "text/html" } }),
  },
});
console.log(`test pages on http://localhost:${PORT}/simple and /heavy`);
